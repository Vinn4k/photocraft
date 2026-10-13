use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use photocraft_codecs::{ChannelLayout, EncodeOptions, Image};
use photocraft_doc::Document;
use photocraft_ui_egui::theme::ThemeKind;
use photocraft_ui_egui::{FileDialogAnswer, FileDialogReply, PhotocraftApp, Services};

use crate::files;
use crate::system::{self, Insets};
use winit::platform::android::activity::AndroidApp;

/// The NativeActivity's entry point (android-activity calls it on its own thread).
// The one item allowed `unsafe_code`: `no_mangle` exports the symbol android-activity links
// against (`extern "Rust" fn android_main(AndroidApp)`); the function itself is safe Rust.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("photocraft"));
    let prefs = app.internal_data_path().map(|p| p.join("preferences.json"));
    let android = app.clone();
    let mut options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    photocraft_ui_egui::gpu_canvas::use_adapter_limits(&mut options.wgpu_options.wgpu_setup);
    let result = eframe::run_native(
        "PhotoCraft",
        options,
        Box::new(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::Pro);
            let queue = files::Queue::default();
            let last_dir = Rc::new(RefCell::new(PathBuf::from(files::STORAGE)));
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services(queue.clone(), last_dir.clone(), prefs));
            app.set_theme(&cc.egui_ctx, ThemeKind::Pro);
            if let Some(rs) = cc.wgpu_render_state.clone() {
                log::info!("photocraft: wgpu backend {:?}", rs.adapter.get_info().backend);
                app.perf.gpu_info.set_adapter(&rs.adapter.get_info());
                app.set_wgpu(rs);
            }
            Ok(Box::new(Shell { app, android, queue, last_dir, insets: None }))
        }),
    );
    if let Err(e) = result {
        log::error!("photocraft: {e}");
    }
}

/// The app, plus what Android needs around it: the system bars kept clear, and PhotoCraft's own
/// file browser for the file dialogs.
struct Shell {
    app: PhotocraftApp,
    android: AndroidApp,
    queue: files::Queue,
    /// Where the file browser opens: the last folder used.
    last_dir: Rc<RefCell<PathBuf>>,
    /// The window size (px) the insets were read at, when they were read (s), and the insets.
    insets: Option<([i32; 2], f64, Insets)>,
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.keep_clear_of_system_bars(ctx, raw_input);
        self.app.raw_input_hook(ctx, raw_input);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(dir) = files::back(&self.queue, &ctx) {
            *self.last_dir.borrow_mut() = dir;
        }
        // The app inside the safe area (the root Ui spans the whole screen); the bars' strips
        // in the interface colour behind the system's icons.
        let safe = ctx.content_rect();
        ui.painter().rect_filled(ctx.viewport_rect(), 0.0, photocraft_ui_egui::theme::Tokens::get(&ctx).chrome);
        ui.scope_builder(egui::UiBuilder::new().max_rect(safe), |ui| self.app.ui(ui, frame));
        let access = files::Access { granted: system::has_file_access, request: ask_file_access, pick: Some(pick_with_android) };
        if let Some(dir) = files::show(&self.queue, &ctx, access) {
            *self.last_dir.borrow_mut() = dir;
        }
    }
}

impl Shell {
    /// NativeActivity draws under the status bar, the navigation bar and the camera cutout (and
    /// the keyboard covers the bottom): give egui those as its safe area, so panels stay inside.
    /// The system's insets (JNI) are read again when the window changes size and twice a
    /// second (the keyboard opens without one). The content rect is only a fallback: it starts
    /// below the theme's (invisible) action bar too, which left a second bar's height empty.
    fn keep_clear_of_system_bars(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let ppp = raw.viewports.get(&raw.viewport_id).and_then(|v| v.native_pixels_per_point).unwrap_or_else(|| ctx.pixels_per_point()) * ctx.zoom_factor();
        let Some(screen) = raw.screen_rect else { return };
        let size = [(screen.width() * ppp).round() as i32, (screen.height() * ppp).round() as i32];
        let now = raw.time.unwrap_or(0.0);
        let stale = self.insets.is_none_or(|(at, when, _)| at != size || now - when > 0.5);
        if stale {
            let cr = self.android.content_rect();
            let from_rect = if cr.right > cr.left && cr.bottom > cr.top { [cr.left, cr.top, size[0] - cr.right, size[1] - cr.bottom] } else { [0; 4] };
            let from_system = system::insets().inspect_err(|e| log::warn!("photocraft: window insets: {e}")).ok();
            let insets: Insets = from_system.unwrap_or(from_rect).map(|v| v.max(0));
            self.insets = Some((size, now, insets));
        }
        if let Some((_, _, [left, top, right, bottom])) = self.insets {
            let pt = |px: i32| px as f32 / ppp;
            raw.safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 { left: pt(left), right: pt(right), top: pt(top), bottom: pt(bottom) }));
        }
    }
}

/// File › Open through Android's photo picker or its file picker (robius-file-picker: its small
/// Java helper receives the picker's answer, which a NativeActivity can't). The picked file is
/// read on the picker's thread and handed to the app by name and contents.
fn pick_with_android(picker: files::Picker, reply: FileDialogReply) -> Result<(), files::PickFailed> {
    let slot = Arc::new(Mutex::new(Some(reply)));
    let answer_slot = slot.clone();
    let done = move |result: robius_file_picker::Result<Option<robius_file_picker::PickedFile>>| {
        let answer = match result {
            Ok(None) => None,
            Ok(Some(file)) => Some(match file.read_bytes() {
                Ok(bytes) => FileDialogAnswer::Contents(picked_name(&file), bytes),
                Err(e) => FileDialogAnswer::Unavailable(format!("Couldn't read the picked file: {e}")),
            }),
            Err(e) => Some(FileDialogAnswer::Unavailable(e.to_string())),
        };
        if let Some(reply) = answer_slot.lock().unwrap_or_else(PoisonError::into_inner).take() {
            reply.send(answer);
        }
    };
    let dialog = robius_file_picker::FileDialog::new();
    let shown = match picker {
        files::Picker::Gallery => dialog.pick_image(done),
        files::Picker::Files => dialog.pick_file(done),
    };
    shown.map_err(|e| (e.to_string(), slot.lock().unwrap_or_else(PoisonError::into_inner).take()))
}

/// The picked file's name with an extension the importer knows (a gallery may name a photo
/// without one): from its MIME type when missing.
fn picked_name(file: &robius_file_picker::PickedFile) -> String {
    let name = file.file_name().or(file.display_name()).unwrap_or("image").to_string();
    if Path::new(&name).extension().is_some() {
        return name;
    }
    let ext = match file.mime_type().unwrap_or_default() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/heic" | "image/heif" => "heic",
        "image/tiff" => "tif",
        "image/bmp" => "bmp",
        "image/vnd.adobe.photoshop" => "psd",
        "image/svg+xml" => "svg",
        "image/x-adobe-dng" => "dng",
        _ => return name,
    };
    format!("{name}.{ext}")
}

/// Ask for file access and say what to do in what opened.
fn ask_file_access() -> Result<String, String> {
    let t = photocraft_ui_egui::i18n::t;
    Ok(match system::request_file_access()? {
        system::Asked::AllFilesSwitch => t("Turn on \"Allow access to manage all files\", then come back to PhotoCraft.").to_string(),
        system::Asked::AllFilesList => t("Find PhotoCraft in the list, turn on its access, then come back.").to_string(),
        system::Asked::AppInfo => t(files::MANUAL).to_string(),
        system::Asked::Permission => t("Allow PhotoCraft to access photos and files when Android asks.").to_string(),
    })
}

fn services(queue: files::Queue, last_dir: Rc<RefCell<PathBuf>>, prefs: Option<PathBuf>) -> Services {
    let prefs_save = prefs.clone();
    Services {
        import: Some(Box::new(|name: &str, bytes: &[u8], max_svg_group_depth: usize| {
            photocraft_io::import_with_svg_group_depth(name, bytes, max_svg_group_depth).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
        })),
        export: Some(Box::new(|doc: &Document, path: &str, settings: &photocraft_ui_egui::ExportSettings| {
            let mut opts = photocraft_io::ExportOptions::default();
            if let Some(q) = settings.jpeg_quality {
                opts.encode.jpeg_quality = q;
            }
            opts.encode.webp_lossless = settings.webp_lossless;
            if let Some(q) = settings.webp_quality {
                opts.encode.webp_quality = q;
            }
            opts.tiff_layers = settings.tiff_layers;
            opts.xmp = if settings.xmp_all { photocraft_io::XmpEmbed::All } else { photocraft_io::XmpEmbed::None };
            opts.tga_bits = settings.tga_bits;
            opts.image_previews = settings.image_previews;
            opts.psd_maximize_compatibility = settings.psd_maximize_compatibility;
            photocraft_io::export(doc, path, &opts).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string())
        })),
        // PhotoCraft's own browser (files.rs), drawn by the shell until the user picks.
        file_dialog: Some(files::service(queue, last_dir)),
        write: Some(Box::new(|path: &str, bytes: &[u8]| write_file(Path::new(path), bytes))),
        encode_png: Some(Box::new(|w, h, rgba| {
            let img = Image::from_u8(w, h, ChannelLayout::Rgba, rgba.to_vec()).map_err(|e| e.to_string())?;
            photocraft_codecs::encode(&img, photocraft_codecs::Format::Png, &EncodeOptions::default()).map_err(|e| e.to_string())
        })),
        load_prefs: Some(Box::new(move || std::fs::read_to_string(prefs.as_ref()?).ok())),
        save_prefs: Some(Box::new(move |text: &str| write_file(prefs_save.as_deref().ok_or("no private storage")?, text.as_bytes()))),
        ..Default::default()
    }
}

/// Write through a temporary file next to `path`, so an interrupted write keeps the old file.
fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("photocraft-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

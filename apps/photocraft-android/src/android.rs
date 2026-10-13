use std::path::{Path, PathBuf};

use photocraft_codecs::{ChannelLayout, EncodeOptions, Image};
use photocraft_doc::Document;
use photocraft_ui_egui::theme::ThemeKind;
use photocraft_ui_egui::{FileDialogAnswer, FileDialogRequest, PhotocraftApp, Services};
use winit::platform::android::activity::AndroidApp;

/// The NativeActivity's entry point (android-activity calls it on its own thread).
// The one item allowed `unsafe_code`: `no_mangle` exports the symbol android-activity links
// against (`extern "Rust" fn android_main(AndroidApp)`); the function itself is safe Rust.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("photocraft"));
    let files = app.external_data_path().or_else(|| app.internal_data_path());
    let prefs = app.internal_data_path().map(|p| p.join("preferences.json"));
    let mut options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    photocraft_ui_egui::gpu_canvas::use_adapter_limits(&mut options.wgpu_options.wgpu_setup);
    let result = eframe::run_native(
        "PhotoCraft",
        options,
        Box::new(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::Pro);
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services(files, prefs));
            app.set_theme(&cc.egui_ctx, ThemeKind::Pro);
            if let Some(rs) = cc.wgpu_render_state.clone() {
                log::info!("photocraft: wgpu backend {:?}", rs.adapter.get_info().backend);
                app.perf.gpu_info.set_adapter(&rs.adapter.get_info());
                app.set_wgpu(rs);
            }
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = result {
        log::error!("photocraft: {e}");
    }
}

fn services(files: Option<PathBuf>, prefs: Option<PathBuf>) -> Services {
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
        file_dialog: Some(Box::new(move |request, _parent, reply| {
            let answer = match request {
                FileDialogRequest::Open { .. } | FileDialogRequest::Folder { .. } => {
                    FileDialogAnswer::Unavailable("Opening files on Android comes with the system file picker; for now, create a new document.".into())
                }
                // No save dialog yet: the suggested name, in the app's folder.
                FileDialogRequest::Save { suggested } => match &files {
                    Some(dir) => {
                        let name = Path::new(&suggested).file_name().map_or_else(|| suggested.clone(), |n| n.to_string_lossy().to_string());
                        FileDialogAnswer::SaveTo(dir.join(name).to_string_lossy().to_string())
                    }
                    None => FileDialogAnswer::Unavailable("This device gives PhotoCraft no folder to save in.".into()),
                },
            };
            reply.send(Some(answer));
        })),
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

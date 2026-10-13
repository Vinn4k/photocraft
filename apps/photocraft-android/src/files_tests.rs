use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui_kittest::kittest::Queryable;
use photocraft_codecs::{ChannelLayout, EncodeOptions, Format, Image};
use photocraft_ui_egui::{PhotocraftApp, Services};

use super::*;

const PHONE: egui::Vec2 = egui::vec2(412.0, 892.0);
static GRANTED: AtomicBool = AtomicBool::new(true);
static ASKED: AtomicBool = AtomicBool::new(false);
/// The tests share GRANTED: one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn access() -> Access {
    Access {
        granted: || GRANTED.load(Ordering::Relaxed),
        request: || {
            ASKED.store(true, Ordering::Relaxed);
            Ok("Settings opened".into())
        },
        pick: Some(|picker, reply| {
            if PICK_FAILS.load(Ordering::Relaxed) {
                return Err(("no picker on this phone".into(), Some(reply)));
            }
            // As the Gallery answers: the picked file's name and contents.
            let name = if picker == Picker::Gallery { "IMG_0001.png" } else { "scan.png" };
            reply.send(Some(FileDialogAnswer::Contents(name.into(), png(5, 3))));
            Ok(())
        }),
    }
}

static PICK_FAILS: AtomicBool = AtomicBool::new(false);

/// The Android shell's frame without the system parts: the app, then the browser over it.
struct Shell {
    app: PhotocraftApp,
    queue: Queue,
    last_dir: Rc<RefCell<PathBuf>>,
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(dir) = back(&self.queue, &ctx) {
            *self.last_dir.borrow_mut() = dir;
        }
        self.app.ui(ui, frame);
        if let Some(dir) = show(&self.queue, &ctx, access()) {
            *self.last_dir.borrow_mut() = dir;
        }
    }
}

/// A folder of its own under the system temp dir, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("photocraft-android-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Album")).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = Image::from_u8(w, h, ChannelLayout::Rgba, vec![200; (w * h * 4) as usize]).unwrap();
    photocraft_codecs::encode(&img, Format::Png, &EncodeOptions::default()).unwrap()
}

fn harness(start: PathBuf) -> egui_kittest::Harness<'static, Shell> {
    let queue = Queue::default();
    let last_dir = Rc::new(RefCell::new(start));
    egui_kittest::Harness::builder().with_size(PHONE).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let services = Services {
            import: Some(Box::new(|name: &str, bytes: &[u8], depth: usize| {
                photocraft_io::import_with_svg_group_depth(name, bytes, depth).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
            })),
            export: Some(Box::new(|doc: &photocraft_doc::Document, path: &str, _settings: &photocraft_ui_egui::ExportSettings| {
                photocraft_io::export(doc, path, &photocraft_io::ExportOptions::default()).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string())
            })),
            write: Some(Box::new(|path: &str, bytes: &[u8]| std::fs::write(path, bytes).map_err(|e| e.to_string()))),
            file_dialog: Some(service(queue.clone(), last_dir.clone())),
            ..Default::default()
        };
        Shell { app: PhotocraftApp::new(photocraft_engine::Session::new(), services), queue: queue.clone(), last_dir: last_dir.clone() }
    })
}

#[test]
fn open_lists_images_and_folders_and_opens_the_one_tapped() {
    let _serial = serial();
    GRANTED.store(true, Ordering::Relaxed);
    let tmp = TempDir::new("open");
    std::fs::write(tmp.0.join("Album/moon.png"), png(6, 4)).unwrap();
    std::fs::write(tmp.0.join("notes.txt"), "not an image").unwrap();
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.open_dialog_file().unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Album").is_some(), "folders are listed");
    assert!(h.query_by_label("notes.txt").is_none(), "only what File › Open reads");
    h.get_by_label("Album").click();
    h.run_steps(4);
    h.get_by_label("moon.png").click();
    h.run_steps(8);
    let st = h.state().app.session.active().expect("the image opened");
    assert_eq!((st.doc.size.width, st.doc.size.height), (6, 4));
    assert!(h.state().queue.borrow().is_none(), "the browser closed");
    assert_eq!(*h.state().last_dir.borrow(), tmp.0.join("Album"), "next time it starts there");
}

#[test]
fn save_as_writes_the_typed_name_in_the_folder_shown() {
    let _serial = serial();
    GRANTED.store(true, Ordering::Relaxed);
    let tmp = TempDir::new("save");
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.run("file.new", serde_json::json!({"width": 8, "height": 8})).unwrap();
    h.state_mut().app.save_as(None).unwrap();
    h.run_steps(4);
    h.get_by_label("Album").click();
    h.run_steps(4);
    // The name field starts with the suggested name; set it as typing would.
    h.state_mut().queue.borrow_mut().as_mut().unwrap().name = "poster.png".into();
    h.run_steps(2);
    h.get_by_label("Save").click();
    h.run_steps(8);
    assert!(tmp.0.join("Album/poster.png").is_file(), "{:?}", std::fs::read_dir(tmp.0.join("Album")).map(|d| d.count()));
}

#[test]
fn without_access_it_asks_for_it_and_back_cancels() {
    let _serial = serial();
    GRANTED.store(false, Ordering::Relaxed);
    let tmp = TempDir::new("access");
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.open_dialog_file().unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Album").is_none(), "nothing listed without access");
    h.get_by_label("Allow Access").click();
    h.run_steps(2);
    assert!(ASKED.load(Ordering::Relaxed), "asked Android for access");
    assert!(h.query_by_label("Settings opened").is_some(), "says what happened");
    h.key_press(egui::Key::BrowserBack);
    h.run_steps(4);
    assert!(h.state().queue.borrow().is_none(), "Back closes it");
    assert!(h.state().app.session.active().is_none());
    GRANTED.store(true, Ordering::Relaxed);
}

/// `PHOTOCRAFT_SNAPSHOT_DIR=… cargo test -p photocraft-android snapshot` writes what it looks like.
#[test]
fn snapshot() {
    let Some(out) = std::env::var_os("PHOTOCRAFT_SNAPSHOT_DIR") else { return };
    let _serial = serial();
    GRANTED.store(true, Ordering::Relaxed);
    let tmp = TempDir::new("snap");
    std::fs::write(tmp.0.join("Album/moon.png"), png(2, 2)).unwrap();
    std::fs::write(tmp.0.join("cartaz.psd"), b"8BPS").unwrap();
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.open_dialog_file().unwrap();
    h.run_steps(6);
    h.render().unwrap().save(PathBuf::from(out).join("android-files.png")).unwrap();
}

#[test]
fn open_offers_androids_gallery_and_file_pickers() {
    let _serial = serial();
    GRANTED.store(false, Ordering::Relaxed);
    PICK_FAILS.store(false, Ordering::Relaxed);
    let tmp = TempDir::new("gallery");
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.open_dialog_file().unwrap();
    h.run_steps(4);
    // No file access needed for Android's own pickers.
    h.get_by_label("Gallery").click();
    h.run_steps(8);
    let st = h.state().app.session.active().expect("the picked photo opened");
    assert_eq!((st.doc.size.width, st.doc.size.height), (5, 3));
    assert!(h.state().queue.borrow().is_none(), "the browser closed");
    GRANTED.store(true, Ordering::Relaxed);
}

#[test]
fn a_picker_that_cannot_open_leaves_the_browser_with_why() {
    let _serial = serial();
    GRANTED.store(true, Ordering::Relaxed);
    PICK_FAILS.store(true, Ordering::Relaxed);
    let tmp = TempDir::new("nopicker");
    std::fs::write(tmp.0.join("Album/moon.png"), png(6, 4)).unwrap();
    let mut h = harness(tmp.0.clone());
    h.state_mut().app.open_dialog_file().unwrap();
    h.run_steps(4);
    h.get_by_label("Files").click();
    h.run_steps(4);
    assert!(h.query_by_label("no picker on this phone").is_some(), "says why");
    // The folders still work.
    h.get_by_label("Album").click();
    h.run_steps(4);
    h.get_by_label("moon.png").click();
    h.run_steps(8);
    assert!(h.state().app.session.active().is_some());
    PICK_FAILS.store(false, Ordering::Relaxed);
}

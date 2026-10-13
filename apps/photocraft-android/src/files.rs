//! PhotoCraft's own file browser on Android, for File › Open, Save As and Export: NativeActivity
//! can't receive the system picker's answer, so the app lists the phone's folders itself (with
//! "All files access", `system.rs`). It answers the app's file dialog request
//! ([`FileDialogRequest`]) through its [`FileDialogReply`].

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use egui::{RichText, Sense, vec2};
use photocraft_ui_egui::i18n::t;
use photocraft_ui_egui::icons;
use photocraft_ui_egui::theme::Tokens;
use photocraft_ui_egui::{FileDialogAnswer, FileDialogReply, FileDialogRequest};

/// The phone's shared storage (what the Files app shows as "Internal storage").
pub const STORAGE: &str = "/storage/emulated/0";

/// Everything File › Open reads (the web build's list): documents, flat images, raw photos and
/// Photoshop presets.
const OPEN_EXTS: &[&str] = &[
    "pcraft", "pdn", "ora", "psd", "psb", "psdt", "png", "jpg", "jpeg", "tif", "tiff", "webp", "gif", "bmp", "tga", "ico", "qoi", "exr", "hdr", "pbm", "pgm",
    "ppm", "pam", "pfm", "dds", "fits", "fit", "fts", "dpx", "heic", "heif", "hif", "dng", "cr2", "cr3", "nef", "nrw", "arw", "pef", "orf", "rw2", "raf",
    "abr", "grd", "svg", "svgz", "aco", "ase", "af", "afdesign", "afphoto", "afpub",
];

/// The folders offered as shortcuts, under [`STORAGE`].
const PLACES: &[(&str, &str)] = &[("Internal storage", ""), ("Pictures", "Pictures"), ("Download", "Download"), ("DCIM", "DCIM"), ("Documents", "Documents")];

/// Where the switch is, when nothing opens it.
pub const MANUAL: &str = "If nothing opens: Settings › Apps › Special app access › All files access › PhotoCraft.";

/// Row and button height (dp), as the rest of the phone layout.
const ROW: f32 = 48.0;

/// The dialog being answered, shared between the file dialog service and the shell's frame.
pub type Queue = Rc<RefCell<Option<Browser>>>;

/// How the browser checks and asks for file access (`system.rs` on Android; fixed in tests).
#[derive(Clone, Copy)]
pub struct Access {
    pub granted: fn() -> bool,
    /// Ask; `Ok` says what to do in what opened, `Err` why nothing did.
    pub request: fn() -> Result<String, String>,
}

/// The app's file dialog service: queue the request for [`show`] to draw. One at a time, as the
/// app asks.
pub fn service(queue: Queue, last_dir: Rc<RefCell<PathBuf>>) -> photocraft_ui_egui::FileDialogFn {
    Box::new(move |request, _parent, reply| {
        let mut slot = queue.borrow_mut();
        if slot.is_some() {
            reply.send(None);
            return;
        }
        *slot = Some(Browser::new(request, reply, &last_dir.borrow()));
    })
}

pub struct Browser {
    request: FileDialogRequest,
    reply: Option<FileDialogReply>,
    dir: PathBuf,
    /// The file name (Save) being typed.
    name: String,
    entries: Option<Result<Vec<Entry>, String>>,
    /// What asking for access said.
    notice: Option<String>,
}

struct Entry {
    name: String,
    dir: bool,
}

impl Browser {
    pub fn new(request: FileDialogRequest, reply: FileDialogReply, last_dir: &Path) -> Self {
        let (dir, name) = match &request {
            FileDialogRequest::Open { initial_dir, .. } | FileDialogRequest::Folder { initial_dir } => {
                (initial_dir.as_ref().map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or_else(|| last_dir.to_path_buf()), String::new())
            }
            FileDialogRequest::Save { suggested } => {
                let p = Path::new(suggested);
                let dir = p.parent().filter(|d| d.is_absolute() && d.is_dir()).map(Path::to_path_buf).unwrap_or_else(|| last_dir.to_path_buf());
                (dir, p.file_name().map_or_else(|| suggested.clone(), |n| n.to_string_lossy().into_owned()))
            }
        };
        Browser { request, reply: Some(reply), dir, name, entries: None, notice: None }
    }

    fn answer(&mut self, answer: Option<FileDialogAnswer>) {
        if let Some(reply) = self.reply.take() {
            reply.send(answer);
        }
    }

    fn go(&mut self, dir: PathBuf) {
        self.dir = dir;
        self.entries = None;
    }

    fn shows(&self, name: &str) -> bool {
        let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match &self.request {
            FileDialogRequest::Open { extensions: Some(exts), .. } => exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)),
            FileDialogRequest::Open { extensions: None, .. } => OPEN_EXTS.contains(&ext.as_str()),
            FileDialogRequest::Folder { .. } => false,
            FileDialogRequest::Save { .. } => true,
        }
    }

    fn list(&self) -> Result<Vec<Entry>, String> {
        let read = std::fs::read_dir(&self.dir).map_err(|e| e.to_string())?;
        let mut entries: Vec<Entry> = read
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let dir = e.file_type().is_ok_and(|t| t.is_dir()) || e.path().is_dir();
                (!name.starts_with('.') && (dir || self.shows(&name))).then_some(Entry { name, dir })
            })
            .collect();
        entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(entries)
    }
}

/// Draw the browser over the app while a dialog waits. Returns the folder the user was in when it
/// closed (to start there next time).
pub fn show(queue: &Queue, ctx: &egui::Context, permission: Access) -> Option<PathBuf> {
    let mut slot = queue.borrow_mut();
    let browser = slot.as_mut()?;
    let access = (permission.granted)();
    if browser.entries.is_none() && access {
        browser.entries = Some(browser.list());
    }
    let tk = Tokens::get(ctx);
    let rect = ctx.content_rect();
    let mut done: Option<Option<FileDialogAnswer>> = None;
    egui::Area::new(egui::Id::new("android-file-browser")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        egui::Frame::NONE.fill(tk.dock).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
            ui.set_min_size(rect.size() - vec2(24.0, 16.0));
            ui.set_max_width(rect.width() - 24.0);
            ui.spacing_mut().button_padding = vec2(12.0, 10.0);
            ui.spacing_mut().interact_size.y = ROW;
            let title = match &browser.request {
                FileDialogRequest::Open { .. } => t("Open"),
                FileDialogRequest::Folder { .. } => t("Choose Folder"),
                FileDialogRequest::Save { .. } => t("Save As"),
            };
            ui.horizontal(|ui| {
                if icon_button(ui, "x", t("Cancel"), &tk).clicked() {
                    done = Some(None);
                }
                ui.label(RichText::new(title).size(18.0).strong().color(tk.text));
            });
            if !access {
                ui.add_space(12.0);
                ui.label(RichText::new(t("PhotoCraft needs access to your files to open and save them. Turn on \"Allow access to manage all files\" for PhotoCraft, then come back.")).color(tk.text));
                ui.add_space(8.0);
                if ui.add(egui::Button::new(t("Allow Access")).min_size(vec2(0.0, ROW))).clicked() {
                    browser.notice = Some((permission.request)().unwrap_or_else(|e| {
                        log::error!("photocraft: file access request: {e}");
                        format!("{} ({e})", t(MANUAL))
                    }));
                }
                ui.add_space(8.0);
                ui.label(RichText::new(browser.notice.as_deref().unwrap_or(t(MANUAL))).color(tk.text_dim));
                return;
            }
            // Shortcuts, then the folder with Up.
            egui::ScrollArea::horizontal().id_salt("android-file-places").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (label, sub) in PLACES {
                        let dir = Path::new(STORAGE).join(sub);
                        if ui.add(egui::Button::selectable(browser.dir == dir, t(label)).min_size(vec2(0.0, 40.0))).clicked() {
                            browser.go(dir);
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                let parent = browser.dir.parent().map(Path::to_path_buf).filter(|p| p.starts_with(STORAGE) || !browser.dir.starts_with(STORAGE));
                if parent.is_some()
                    && icon_button(ui, up_icon(), t("Up"), &tk).clicked()
                    && let Some(p) = parent
                {
                    browser.go(p);
                }
                let shown = browser.dir.strip_prefix(STORAGE).map_or_else(|_| browser.dir.display().to_string(), |p| format!("{}/{}", t("Internal storage"), p.display()));
                ui.label(RichText::new(shown).color(tk.text_dim));
            });
            let save = matches!(browser.request, FileDialogRequest::Save { .. });
            let folder = matches!(browser.request, FileDialogRequest::Folder { .. });
            if save || folder {
                ui.horizontal(|ui| {
                    if save {
                        ui.add(egui::TextEdit::singleline(&mut browser.name).desired_width(ui.available_width() - 110.0).margin(vec2(10.0, 12.0)));
                    }
                    let label = if save { t("Save") } else { t("Choose") };
                    let ok = !save || !browser.name.trim().is_empty();
                    if ui.add_enabled(ok, egui::Button::new(label).min_size(vec2(96.0, ROW))).clicked() {
                        done = Some(Some(if save {
                            FileDialogAnswer::SaveTo(browser.dir.join(browser.name.trim()).to_string_lossy().into_owned())
                        } else {
                            FileDialogAnswer::FolderTo(browser.dir.to_string_lossy().into_owned())
                        }));
                    }
                });
            }
            ui.separator();
            let mut open_dir = None;
            egui::ScrollArea::vertical().id_salt("android-file-list").auto_shrink([false, false]).show(ui, |ui| match &browser.entries {
                Some(Ok(entries)) if entries.is_empty() => {
                    ui.label(RichText::new(t("No files here")).color(tk.text_dim));
                }
                Some(Ok(entries)) => {
                    for entry in entries {
                        if row(ui, entry, &tk).clicked() {
                            let path = browser.dir.join(&entry.name);
                            if entry.dir {
                                open_dir = Some(path);
                            } else if save {
                                browser.name = entry.name.clone();
                            } else {
                                done = Some(Some(FileDialogAnswer::Paths(vec![path.to_string_lossy().into_owned()])));
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(e).color(tk.text_dim));
                }
                None => {}
            });
            if let Some(dir) = open_dir {
                browser.go(dir);
            }
        });
    });
    let answer = done?;
    browser.answer(answer);
    let dir = browser.dir.clone();
    *slot = None;
    Some(dir)
}

/// Android's Back while the browser is open: cancel it, as the system pickers do. Call before the
/// app's frame, which would take Back for its own navigation. Returns the folder it was in.
pub fn back(queue: &Queue, ctx: &egui::Context) -> Option<PathBuf> {
    let mut slot = queue.borrow_mut();
    let browser = slot.as_mut()?;
    if !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) {
        return None;
    }
    browser.answer(None);
    let dir = browser.dir.clone();
    *slot = None;
    Some(dir)
}

fn up_icon() -> &'static str {
    ["arrow-up", "chevron-up", "chevron-left"].into_iter().find(|n| icons::exists(n)).unwrap_or("chevron-left")
}

fn icon_button(ui: &mut egui::Ui, icon: &str, label: &str, tk: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ROW, ROW), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, tk.radius, tk.row_selected);
    }
    icons::paint(ui, egui::Rect::from_center_size(rect.center(), vec2(22.0, 22.0)), icon, 22.0, tk.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// A folder or file: its icon and name, the whole width.
fn row(ui: &mut egui::Ui, entry: &Entry, tk: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, tk.radius, tk.row_selected);
    }
    let icon = if entry.dir { "folder-open" } else { "image" };
    icons::paint(
        ui,
        egui::Rect::from_center_size(egui::pos2(rect.left() + 22.0, rect.center().y), vec2(22.0, 22.0)),
        icon,
        22.0,
        if entry.dir { tk.accent } else { tk.text_dim },
    );
    let galley = ui.painter().layout(entry.name.clone(), egui::FontId::proportional(15.0), tk.text, rect.width() - 56.0);
    ui.painter().galley(egui::pos2(rect.left() + 48.0, rect.center().y - galley.size().y / 2.0), galley, tk.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &entry.name));
    resp
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;

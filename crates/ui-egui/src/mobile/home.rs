//! Home (the phone's start screen): New Document and Open… up front, then the recent files in
//! a two-column grid. Opening or creating a document goes to the Editor (`MobileState::sync`).

use egui::{RichText, Sense, Stroke, StrokeKind, vec2};

use super::{Route, touch};
use crate::PhotocraftApp;
use crate::theme::Tokens;

/// Recent files listed (File › Open Recent keeps more).
pub const RECENT: usize = 8;
const GAP: f32 = 12.0;
const PAD: f32 = 16.0;

pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("mobile-home-bar")
        .exact_size(touch::BAR)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: 16, right: 8, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            crate::direction::horizontal_centered(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                crate::icons::paint(ui, r, "image", 24.0, t.accent);
                ui.label(RichText::new(tl!("PhotoCraft")).font(crate::theme::semibold(18.0)).color(t.text));
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.dock).inner_margin(egui::Margin { left: 16, right: 16, top: 20, bottom: 16 })).show(
        ui,
        |ui| {
            egui::ScrollArea::vertical().id_salt("mobile-home").show(ui, |ui| {
                let col = ((ui.available_width() - GAP) / 2.0).max(80.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    if touch::action_card(ui, "file-plus", tl!("New Document"), col, true).clicked() {
                        app.ui.mobile.go(Route::NewDocument);
                    }
                    // Android's Open goes through the system file picker (SAF, phase 5); here the
                    // platform's file dialog.
                    if touch::action_card(ui, "folder-open", tl!("Open…"), col, false).clicked()
                        && let Err(e) = app.open_dialog_file()
                    {
                        app.ui.status = e;
                    }
                });
                ui.add_space(20.0);
                recent(app, ui, col, &t);
            });
        },
    );
}

/// The recent files, two to a row: a tile with the file's kind, its name and folder. A tap opens
/// it; a long press (or right click) offers Remove from Recent and Clear Recent Files.
fn recent(app: &mut PhotocraftApp, ui: &mut egui::Ui, col: f32, t: &Tokens) {
    // On the web there are no paths to reopen.
    let files: Vec<String> = if cfg!(target_arch = "wasm32") { Vec::new() } else { app.ui.recent_files.iter().take(RECENT).cloned().collect() };
    ui.label(RichText::new(tl!("Recent")).font(crate::theme::semibold(15.0)).color(t.text));
    ui.add_space(4.0);
    if files.is_empty() {
        ui.label(RichText::new(tl!("Create a new document or open an existing file.")).color(t.text_dim));
        return;
    }
    let (mut open, mut remove, mut clear) = (None, None, false);
    for row in files.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for path in row {
                let name = crate::file_open::display_name(path);
                let folder = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
                let (rect, resp) = ui.allocate_exact_size(vec2(col, 168.0 + 46.0), Sense::click());
                let tile = egui::Rect::from_min_size(rect.min, vec2(col, 168.0));
                ui.painter().rect_filled(tile, t.radius_lg, t.canvas);
                if resp.is_pointer_button_down_on() {
                    ui.painter().rect_stroke(tile, t.radius_lg, Stroke::new(2.0, t.accent), StrokeKind::Inside);
                }
                crate::icons::paint(ui, egui::Rect::from_center_size(tile.center() - vec2(0.0, 10.0), vec2(40.0, 40.0)), "image", 40.0, t.text_faint);
                let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
                ui.painter().text(tile.center() + vec2(0.0, 26.0), egui::Align2::CENTER_CENTER, ext, crate::theme::mono(11.0), t.text_faint);
                let x = rect.left() + 2.0;
                let name_g = crate::tab_strip::elided(ui, &name, egui::FontId::proportional(14.0), t.text, col - 4.0);
                ui.painter().galley(egui::pos2(x, tile.bottom() + 8.0), name_g, t.text);
                let folder_g = crate::tab_strip::elided(ui, &folder, egui::FontId::proportional(12.0), t.text_dim, col - 4.0);
                ui.painter().galley(egui::pos2(x, tile.bottom() + 28.0), folder_g, t.text_dim);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                if resp.clicked() {
                    open = Some(path.clone());
                }
                resp.context_menu(|ui| {
                    touch::style(ui);
                    if ui.button(tl!("Remove from Recent")).clicked() {
                        remove = Some(path.clone());
                        ui.close();
                    }
                    if ui.button(tl!("Clear Recent Files")).clicked() {
                        clear = true;
                        ui.close();
                    }
                });
            }
        });
        ui.add_space(PAD - 4.0);
    }
    if clear {
        app.clear_recent();
    } else if let Some(path) = remove {
        app.remove_recent(&path);
    }
    if let Some(path) = open
        && let Err(e) = app.open_path(&path)
    {
        app.open_failed(&crate::file_open::display_name(&path), &e);
    }
}

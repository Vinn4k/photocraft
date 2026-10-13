//! New Document on a phone: Photoshop's categories as tabs, the blank-document presets three to
//! a row, then the Preset Details. It edits the same fields as the desktop dialog
//! (`new_doc_ui`), kept in `MobileState::new_document`, and Create goes through the same
//! [`crate::new_doc_ui::create`].

use egui::{RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Map, Value, json};

use super::{Route, touch};
use crate::PhotocraftApp;
use crate::new_doc_ui::{self as nd, CATEGORIES, Card};
use crate::theme::Tokens;

const CARD_H: f32 = 104.0;
const GAP: f32 = 10.0;

pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if app.ui.mobile.new_document.is_empty() {
        app.ui.mobile.new_document = app.new_document_fields();
    }
    let t = Tokens::get(ui.ctx());
    let mut create = false;
    egui::Panel::top("mobile-new-document-bar")
        .exact_size(touch::BAR)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(8, 0)))
        .show(ui, |ui| {
            crate::direction::horizontal_centered(ui, |ui| {
                if touch::icon_button(ui, "x", tl!("Close"), false).clicked() {
                    app.ui.mobile.back();
                }
                ui.label(RichText::new(tl!("New Document")).font(crate::theme::semibold(17.0)).color(t.text));
                ui.with_layout(crate::direction::trailing(), |ui| {
                    create = touch::pill(ui, tl!("Create")).clicked();
                });
            });
        });
    let mut f = std::mem::take(&mut app.ui.mobile.new_document);
    categories(ui, &mut f, &t);
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.dock).inner_margin(egui::Margin::same(16))).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("mobile-new-document").show(ui, |ui| {
            presets(app, ui, &mut f, &t);
            ui.add_space(16.0);
            details(ui, &mut f, &t);
        });
    });
    if create {
        match nd::create(app, &f) {
            // `sync` sees the new document and opens the Editor (Back then returns Home).
            Ok(_) => app.ui.mobile.replace(Route::Editor),
            Err(e) => {
                app.ui.status = e;
                app.ui.mobile.new_document = f;
            }
        }
    } else if app.ui.mobile.route == Route::NewDocument {
        app.ui.mobile.new_document = f;
    }
}

/// The category tabs, scrolling sideways (Recent, Photo, Print, Art & Illustration, Web, Mobile,
/// Film & Video).
fn categories(ui: &mut egui::Ui, f: &mut Map<String, Value>, t: &Tokens) {
    let current = nd::get_s(f, "__category", "Recent");
    egui::Panel::top("mobile-new-document-tabs").exact_size(52.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(8, 0))).show(
        ui,
        |ui| {
            egui::ScrollArea::horizontal().id_salt("mobile-new-document-tabs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (name, _) in CATEGORIES {
                        let on = current == *name;
                        let label = crate::i18n::t(name);
                        let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), if on { t.text } else { t.text_dim });
                        let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, touch::FIELD), Sense::click());
                        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, t.text);
                        if on {
                            ui.painter()
                                .line_segment([rect.left_bottom() + vec2(6.0, -2.0), rect.right_bottom() + vec2(-6.0, -2.0)], Stroke::new(2.0, t.accent));
                        }
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
                        if resp.clicked() {
                            f.insert("__category".into(), json!(name));
                        }
                    }
                });
            });
        },
    );
}

/// The category's presets in a grid of three: page shape, name, size.
fn presets(app: &mut PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>, t: &Tokens) {
    let units = app.session.prefs().units_and_rulers.clone();
    let cat = nd::get_s(f, "__category", "Recent");
    let cards: Vec<Card> = match CATEGORIES.iter().find(|c| c.0 == cat) {
        Some(c) if c.0 != CATEGORIES[0].0 => c.1.iter().copied().map(Card::Builtin).collect(),
        _ => nd::recent_cards(f, &app.session.presets.recent_documents, &units),
    };
    let chosen = nd::get_s(f, "__preset", "");
    let w = ((ui.available_width() - 2.0 * GAP) / 3.0).max(60.0);
    let mut picked = None;
    for row in cards.chunks(3) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for c in row {
                let (r, resp) = ui.allocate_exact_size(vec2(w, CARD_H), Sense::click());
                let (pw, ph, _) = c.size(&units);
                let on = match c {
                    Card::Builtin(p) => chosen == p.0,
                    Card::Recent(s) => nd::holds(f, s),
                };
                ui.painter().rect_filled(r, t.radius_lg, if on { t.row_selected } else { t.chrome });
                ui.painter().rect_stroke(
                    r,
                    t.radius_lg,
                    Stroke::new(if on { 2.0 } else { 1.0 }, if on { t.accent } else { t.field_border }),
                    StrokeKind::Inside,
                );
                nd::page_icon(ui, egui::Rect::from_center_size(pos2(r.center().x, r.top() + 30.0), vec2(40.0, 40.0)), pw, ph, t);
                let title = nd::card_title(ui.painter(), crate::i18n::t(c.title()), w - 10.0, t.text);
                ui.painter().galley(pos2(r.center().x, r.top() + 66.0 - title.size().y / 2.0), title, t.text);
                let size = format!("{pw} × {ph}");
                ui.painter().text(pos2(r.center().x, r.bottom() - 12.0), egui::Align2::CENTER_CENTER, size, crate::theme::mono(10.5), t.text_dim);
                let name = crate::i18n::t(c.title()).to_string();
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, &name));
                if resp.clicked() {
                    picked = Some(match c {
                        Card::Builtin(p) => Ok(*p),
                        Card::Recent(s) => Err(s.clone()),
                    });
                }
            }
        });
        ui.add_space(GAP - 8.0);
    }
    match picked {
        Some(Ok(p)) => nd::apply_preset(f, &p, &units),
        Some(Err(settings)) => nd::apply_settings(f, &settings),
        None => {}
    }
}

/// Preset Details: name, size and unit, resolution, color mode and depth, background.
fn details(ui: &mut egui::Ui, f: &mut Map<String, Value>, t: &Tokens) {
    egui::Frame::NONE.fill(t.chrome).corner_radius(t.radius_lg).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let full = ui.available_width();
        let half = ((full - GAP) / 2.0).max(60.0);
        touch::heading(ui, tl!("PRESET DETAILS"));
        touch::caption(ui, tl!("Name"));
        let mut name = nd::get_s(f, "name", "Untitled-1");
        if touch::text(ui, &mut name, full).changed() {
            f.insert("name".into(), json!(name));
        }
        let ppi = nd::get_f(f, "resolution", 72.0);
        let mut unit = nd::get_s(f, "__unit", "px");
        let decimals = crate::widgets::unit_decimals(&unit) as usize;
        two(ui, half, |ui, which| {
            let key = if which == 0 { "width" } else { "height" };
            touch::caption(ui, if which == 0 { tl!("Width") } else { tl!("Height") });
            let mut v = nd::shown_size(f, key, if which == 0 { 1920.0 } else { 1080.0 }, &unit, ppi);
            let max = nd::to_unit(300_000.0, &unit, ppi);
            if touch::number(ui, &mut v, 0.01..=max, decimals, "", half).changed() {
                nd::set_size(f, key, v, &unit, ppi);
            }
        });
        two(ui, half, |ui, which| {
            if which == 0 {
                touch::caption(ui, tl!("Units"));
                let opts: Vec<(String, &str)> = nd::UNITS.iter().map(|u| (u.0.to_string(), u.1)).collect();
                if touch::choice(ui, "mobile-nd-unit", &mut unit, &opts, half) {
                    f.insert("__unit".into(), json!(unit));
                    f.remove("__savedPreset");
                }
            } else {
                touch::caption(ui, tl!("Resolution"));
                let mut r = ppi;
                if touch::number(ui, &mut r, 1.0..=30_000.0, 2, " ppi", half).changed() {
                    nd::set_resolution(f, r);
                }
            }
        });
        // Page orientation: swap width and height.
        ui.horizontal(|ui| {
            touch::caption(ui, crate::i18n::tr_ctx(crate::i18n::current(), "document", "Orientation"));
            let (w, h) = (nd::get_f(f, "width", 1920.0), nd::get_f(f, "height", 1080.0));
            for (icon, portrait, label) in [("rectangle-vertical", true, tl!("Portrait")), ("rectangle-horizontal", false, tl!("Landscape"))] {
                if touch::icon_button(ui, icon, label, (h > w) == portrait).clicked() && (h > w) != portrait {
                    nd::swap_size(f);
                }
            }
        });
        two(ui, half, |ui, which| {
            if which == 0 {
                touch::caption(ui, tl!("Color Mode"));
                let mut mode = nd::get_s(f, "mode", "rgb");
                let opts =
                    [("gray".to_string(), "Grayscale"), ("rgb".to_string(), "RGB Color"), ("cmyk".to_string(), "CMYK Color"), ("lab".to_string(), "Lab Color")];
                if touch::choice(ui, "mobile-nd-mode", &mut mode, &opts, half) {
                    f.insert("mode".into(), json!(mode));
                    f.remove("__savedPreset");
                }
            } else {
                touch::caption(ui, tl!("Bit depth"));
                let mut depth = f.get("depth").and_then(Value::as_u64).unwrap_or(8);
                let opts: Vec<(u64, &str)> = nd::DEPTH_OPTIONS.iter().map(|(bits, label, _)| (*bits, *label)).collect();
                if touch::choice(ui, "mobile-nd-depth", &mut depth, &opts, half) {
                    f.insert("depth".into(), json!(depth));
                    f.remove("__savedPreset");
                }
            }
        });
        touch::caption(ui, tl!("Background Contents"));
        let mut bg = nd::get_s(f, "background", "white");
        let opts = [
            ("white".to_string(), "White"),
            ("black".to_string(), "Black"),
            ("backgroundColor".to_string(), "Background Color"),
            ("transparent".to_string(), "Transparent"),
        ];
        if touch::choice(ui, "mobile-nd-background", &mut bg, &opts, full) {
            f.insert("background".into(), json!(bg));
            f.remove("__savedPreset");
            // A fresh choice of Background Color uses today's toolbox colour.
            f.remove("backgroundColor");
        }
    });
}

/// Two fields side by side, each `width` wide (`which` is 0 for the left one).
fn two(ui: &mut egui::Ui, width: f32, mut field: impl FnMut(&mut egui::Ui, usize)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        for which in 0..2 {
            ui.allocate_ui_with_layout(vec2(width, 80.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                field(ui, which);
            });
        }
    });
}

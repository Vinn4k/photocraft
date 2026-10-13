//! The Editor on a phone, after the phase 1 wireframes: the current canvas (`canvas::document_area`)
//! full screen with touch bars around it.
//!
//! - **Portrait**: a top bar (Back, the document and its zoom/layer/mode line, Undo, Redo,
//!   Export, Menus), the tool options bar, the canvas with the floating Shift/Alt/Ctrl keys and
//!   the Size/Opacity rail, the tool bar (colors, six tools, all tools) and the panel bar.
//! - **Landscape**: one bar (Back, the document, the tool options, Undo…Menus), the tools down the
//!   left, and on the right the open panel (272 dp) beside a strip of panel buttons; closing the
//!   panel collapses it to the strip.

use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use super::{Orientation, Route, Sheet, touch};
use crate::PhotocraftApp;
use crate::state::Tool;
use crate::theme::Tokens;

/// The tool slots on the tool bar, by their first tool (each slot is a Photoshop toolbar group,
/// its flyout reached by a long press): Move, Marquee, Lasso, Brush, Eraser, Type.
pub const QUICK_SLOTS: [Tool; 6] = [Tool::Move, Tool::RectMarquee, Tool::Lasso, Tool::Brush, Tool::Eraser, Tool::Type];
/// The panel buttons (portrait's bottom bar, landscape's strip), with their icons.
pub const PANEL_BUTTONS: [(Sheet, &str, &str); 5] = [
    (Sheet::Layers, "layers", "Layers"),
    (Sheet::Curves, "sliders-horizontal", "Adjustments"),
    (Sheet::Brush, "brush", "Properties"),
    (Sheet::History, "clock", "History"),
    (Sheet::Color, "palette", "Color"),
];
/// The landscape side panel's width (dp).
pub const SIDE_PANEL: f32 = 272.0;

pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    match Orientation::for_size(ui.ctx().content_rect().size()) {
        Orientation::Portrait => {
            top_bar(app, ui, &t);
            crate::panels::options_bar(app, ui);
            panel_bar(app, ui, &t);
            tool_bar(app, ui, &t);
            if let Some(sheet) = app.ui.mobile.sheet {
                super::sheet_panel(app, ui, &t, sheet);
            }
        }
        Orientation::Landscape => {
            landscape_bar(app, ui, &t);
            tool_column(app, ui, &t);
            side_panel(app, ui, &t);
        }
    }
    let canvas = egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| crate::canvas::document_area(app, ui)).response.rect;
    // A portrait sheet covers most of the canvas: the floating keys and rail wait until it closes.
    let covered = app.ui.mobile.sheet.is_some() && Orientation::for_size(ui.ctx().content_rect().size()) == Orientation::Portrait;
    if !covered {
        modifier_keys(app, ui.ctx(), canvas, &t);
        brush_rail(app, ui.ctx(), canvas, &t);
    }
    all_tools(app, ui.ctx(), &t);
}

/// "33.3% (Layer 1, RGB/8)": the document tab's line without the name.
fn status_line(app: &PhotocraftApp) -> String {
    app.session.active_index().map(|i| crate::canvas::doc_status(app, i)).unwrap_or_default()
}

fn top_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::Panel::top("mobile-editor-bar").exact_size(touch::BAR).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(4, 0))).show(
        ui,
        |ui| {
            crate::direction::horizontal_centered(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                back_button(app, ui);
                ui.with_layout(crate::direction::trailing(), |ui| {
                    actions(app, ui, true);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| title(app, ui, t));
                });
            });
        },
    );
}

/// Back to Home (the documents stay open).
fn back_button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if touch::icon_button(ui, "chevron-left", tl!("Back"), false).clicked() {
        app.ui.mobile.back();
    }
}

/// The document's name over its status line, elided to the room left.
fn title(app: &PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some(st) = app.session.active() else { return };
    let name = format!("{}{}", st.doc.name, if st.is_dirty() { "*" } else { "" });
    let w = ui.available_width().max(40.0);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        // Two lines centred in the bar.
        ui.add_space(((ui.available_height() - 36.0) / 2.0).max(0.0));
        let g = crate::tab_strip::elided(ui, &name, crate::theme::semibold(14.0), t.text, w);
        ui.label(g);
        let g = crate::tab_strip::elided(ui, &status_line(app), egui::FontId::proportional(12.0), t.text_dim, w);
        ui.label(g);
    });
}

/// Undo, Redo, Export and Menus, laid out from the trailing edge (`rtl` order: Menus first).
fn actions(app: &mut PhotocraftApp, ui: &mut egui::Ui, trailing: bool) {
    let mut items: Vec<(&str, &str, Action)> = vec![
        ("undo-2", tl!("Undo"), Action::Run("edit.undo")),
        ("redo-2", tl!("Redo"), Action::Run("edit.redo")),
        ("file", tl!("Export"), Action::Sheet(Sheet::Export)),
        ("ellipsis", tl!("Menus"), Action::Sheet(Sheet::Menus)),
    ];
    if trailing {
        items.reverse();
    }
    for (icon, label, action) in items {
        let on = matches!(action, Action::Sheet(s) if app.ui.mobile.sheet == Some(s));
        let enabled = match action {
            Action::Run(id) => crate::menus::is_enabled(app, id),
            Action::Sheet(_) => true,
        };
        let resp = ui.add_enabled_ui(enabled, |ui| touch::icon_button(ui, icon, label, on)).inner;
        if resp.clicked() {
            match action {
                Action::Run(id) => {
                    if let Err(e) = crate::menus::invoke(app, ui.ctx(), id, json!({})) {
                        app.ui.status = e;
                    }
                }
                Action::Sheet(s) => {
                    let _ = app.ui.mobile.toggle_sheet(s);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    Run(&'static str),
    Sheet(Sheet),
}

/// Portrait's bottom bar: Layers, Adjustments, Properties, History, Color.
fn panel_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::Panel::bottom("mobile-panel-bar").exact_size(64.0).frame(egui::Frame::NONE.fill(t.dock)).show(ui, |ui| {
        let w = ui.available_width() / PANEL_BUTTONS.len() as f32;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (sheet, icon, label) in PANEL_BUTTONS {
                let on = app.ui.mobile.sheet == Some(sheet);
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 64.0), Sense::click());
                let color = if on { t.text } else { t.text_dim };
                if on {
                    ui.painter().rect_filled(rect.shrink2(vec2(6.0, 6.0)), t.radius, t.row_selected);
                }
                crate::icons::paint(ui, Rect::from_center_size(rect.center() - vec2(0.0, 9.0), vec2(20.0, 20.0)), icon, 20.0, color);
                let label = crate::i18n::t(label);
                ui.painter().text(rect.center() + vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), color);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
                if resp.clicked() {
                    let _ = app.ui.mobile.toggle_sheet(sheet);
                }
            }
        });
    });
}

/// Portrait's tool bar: the colors, the six tool slots and All Tools.
fn tool_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::Panel::bottom("mobile-tool-bar").exact_size(64.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(6, 0))).show(
        ui,
        |ui| {
            crate::direction::horizontal_centered(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                color_chips(app, ui, t);
                tools(app, ui, t);
            });
        },
    );
}

/// Landscape's tool column.
fn tool_column(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    crate::direction::start_panel("mobile-tool-column")
        .resizable(false)
        .exact_size(60.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(6, 6)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("mobile-tool-column").show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                color_chips(app, ui, t);
                tools(app, ui, t);
            });
        });
}

/// The foreground/background chips: a tap opens the Color panel.
fn color_chips(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let (rect, resp) = ui.allocate_exact_size(vec2(touch::TARGET, touch::TARGET), Sense::click());
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let [fg, bg] = [app.session.tools.foreground, app.session.tools.background].map(|c| Color32::from_rgb(q(c[0]), q(c[1]), q(c[2])));
    let back = Rect::from_min_size(rect.min + vec2(20.0, 20.0), vec2(22.0, 22.0));
    let front = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(22.0, 22.0));
    ui.painter().rect_filled(back, 2.0, bg);
    ui.painter().rect_stroke(back, 2.0, Stroke::new(1.0, t.separator), StrokeKind::Outside);
    ui.painter().rect_filled(front, 2.0, fg);
    ui.painter().rect_stroke(front, 2.0, Stroke::new(1.0, t.text), StrokeKind::Outside);
    let label = tl!("Set foreground color");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if resp.clicked() {
        let _ = app.ui.mobile.toggle_sheet(Sheet::Color);
    }
}

/// The six tool slots and All Tools. A tap picks the slot's tool (the last one used in it); a long
/// press lists the slot's tools.
fn tools(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    for first in QUICK_SLOTS {
        let Some(slot) = crate::panels::tool_slot(first) else { continue };
        let key = egui::Id::new(("mobile-tool-slot", first));
        let tool = remembered(ui, app.ui.tool, slot, key);
        let on = slot.contains(&app.ui.tool);
        let resp = touch::icon_button(ui, crate::icons::tool_icon(tool), tl!(tool.label()), on);
        if slot.len() > 1 {
            crate::panels::paint_toolbar_flyout_indicator(ui, t, &resp);
        }
        if resp.clicked() {
            app.ui.tool = tool;
        }
        if slot.len() > 1 && (resp.long_touched() || resp.secondary_clicked()) {
            egui::Popup::open_id(ui.ctx(), key.with("flyout"));
        }
        egui::Popup::from_response(&resp).id(key.with("flyout")).open_memory(None).show(|ui| {
            touch::style(ui);
            for &item in slot {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::selectable(item == app.ui.tool, tl!(item.label())).min_size(vec2(200.0, touch::FIELD))).clicked() {
                        app.ui.tool = item;
                        ui.close();
                    }
                });
            }
        });
    }
    let open = app.ui.mobile.all_tools;
    if touch::icon_button(ui, "grid-2x2", tl!("Tools"), open).clicked() {
        app.ui.mobile.all_tools = !open;
    }
}

/// The tool a slot shows: the current tool if it is in the slot, else the last one used there.
fn remembered(ui: &egui::Ui, current: Tool, slot: &[Tool], key: egui::Id) -> Tool {
    if slot.contains(&current) {
        ui.data_mut(|d| d.insert_temp(key, current));
        return current;
    }
    ui.data(|d| d.get_temp::<Tool>(key)).filter(|t| slot.contains(t)).unwrap_or(slot.first().copied().unwrap_or(current))
}

/// Every toolbar group's tools in a grid over the canvas; a tap picks one and closes it.
fn all_tools(app: &mut PhotocraftApp, ctx: &egui::Context, t: &Tokens) {
    if !app.ui.mobile.all_tools {
        return;
    }
    let screen = ctx.content_rect();
    let width = (screen.width() - 24.0).min(420.0);
    let mut close = false;
    egui::Area::new(egui::Id::new("mobile-all-tools")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.chrome).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tl!("Tools")).font(crate::theme::semibold(15.0)).color(t.text));
                ui.with_layout(crate::direction::trailing(), |ui| close = touch::icon_button(ui, "x", tl!("Close"), false).clicked());
            });
            let per_row = ((width / touch::TARGET).floor() as usize).max(1);
            egui::ScrollArea::vertical().max_height(screen.height() * 0.6).show(ui, |ui| {
                let all: Vec<Tool> = crate::panels::tool_slots().flat_map(|s| s.iter().copied()).collect();
                for row in all.chunks(per_row) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for &tool in row {
                            if touch::icon_button(ui, crate::icons::tool_icon(tool), tl!(tool.label()), tool == app.ui.tool).clicked() {
                                app.ui.tool = tool;
                                close = true;
                            }
                        }
                    });
                }
            });
        });
    });
    if close {
        app.ui.mobile.all_tools = false;
    }
}

/// Shift, Alt and Ctrl down the canvas's leading edge: each stays down until tapped again, as a
/// held key (the phone has no keyboard). The shell adds them to every input event (`lib.rs`).
fn modifier_keys(app: &mut PhotocraftApp, ctx: &egui::Context, canvas: Rect, t: &Tokens) {
    let x = if crate::direction::is_rtl() { canvas.right() - 8.0 - touch::TARGET } else { canvas.left() + 8.0 };
    let top = canvas.center().y - (3.0 * touch::TARGET + 12.0) / 2.0;
    egui::Area::new(egui::Id::new("mobile-modifier-keys")).order(egui::Order::Middle).fixed_pos(pos2(x, top)).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let keys = &mut app.ui.mobile.modifiers;
        for (label, on) in [("Shift", &mut keys.shift), ("Alt", &mut keys.alt), ("Ctrl", &mut keys.ctrl)] {
            let (rect, resp) = ui.allocate_exact_size(vec2(touch::TARGET, touch::TARGET), Sense::click());
            let (fill, text) = if *on { (t.accent, t.primary_text) } else { (t.chrome.gamma_multiply(0.92), t.text) };
            ui.painter().rect_filled(rect, t.radius_lg, fill);
            ui.painter().rect_stroke(rect, t.radius_lg, Stroke::new(1.0, if *on { t.accent } else { t.field_border }), StrokeKind::Inside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, crate::theme::semibold(12.0), text);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, label));
            if resp.clicked() {
                *on = !*on;
            }
        }
    });
}

/// Size and Opacity on vertical sliders down the canvas's trailing edge, for painting tools.
fn brush_rail(app: &mut PhotocraftApp, ctx: &egui::Context, canvas: Rect, t: &Tokens) {
    if !app.ui.tool.is_brushlike() {
        return;
    }
    let w = 44.0;
    let h = 2.0 * 120.0 + 70.0;
    let x = if crate::direction::is_rtl() { canvas.left() + 8.0 } else { canvas.right() - 8.0 - w };
    let before = app.session.tools.brush.clone();
    let mut brush = before.clone();
    egui::Area::new(egui::Id::new("mobile-brush-rail")).order(egui::Order::Middle).fixed_pos(pos2(x, canvas.center().y - h / 2.0)).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(t.chrome.gamma_multiply(0.92))
            .stroke(Stroke::new(1.0, t.field_border))
            .corner_radius(w / 2.0)
            .inner_margin(egui::Margin::symmetric(0, 10))
            .show(ui, |ui| {
                ui.set_width(w);
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().slider_width = 120.0;
                    ui.label(RichText::new(tl!("Size")).size(11.0).color(t.text_dim));
                    // Only a moved slider writes: the brush may hold values outside the rail's range.
                    let max = photocraft_paint::MAX_BRUSH_SIZE.min(1000.0);
                    let mut size = brush.size;
                    if ui.add(egui::Slider::new(&mut size, 1.0..=max).vertical().logarithmic(true).show_value(false)).changed() {
                        brush.size = size;
                    }
                    ui.add_space(6.0);
                    // The brush keeps opacity as 0..1; the rail shows percent, as the options bar.
                    let mut percent = brush.opacity * 100.0;
                    if ui.add(egui::Slider::new(&mut percent, 1.0..=100.0).vertical().show_value(false)).changed() {
                        brush.opacity = percent / 100.0;
                    }
                    ui.label(RichText::new(tl!("Opacity")).size(11.0).color(t.text_dim));
                });
            });
    });
    if brush != before {
        crate::brush_panel::commit_gesture(app, ctx, &before, &brush);
    }
}

/// Landscape's single bar: Back, the document, the tool options, then Undo…Menus.
fn landscape_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::Panel::top("mobile-editor-bar").exact_size(touch::TARGET).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(4, 0))).show(
        ui,
        |ui| {
            crate::direction::horizontal_centered(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                back_button(app, ui);
                ui.allocate_ui(vec2(150.0, touch::TARGET), |ui| title(app, ui, t));
                ui.with_layout(crate::direction::trailing(), |ui| {
                    actions(app, ui, true);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| crate::panels::options_bar_contents(app, ui));
                });
            });
        },
    );
}

/// Landscape's right side: the open panel beside the strip of panel buttons.
fn side_panel(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let end = !crate::direction::is_rtl();
    let strip = if end { egui::Panel::right("mobile-panel-strip") } else { egui::Panel::left("mobile-panel-strip") };
    strip.resizable(false).exact_size(touch::TARGET + 8.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(4, 6))).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for (sheet, icon, label) in PANEL_BUTTONS {
            let on = app.ui.mobile.sheet == Some(sheet);
            if touch::icon_button(ui, icon, crate::i18n::t(label), on).clicked() {
                let _ = app.ui.mobile.toggle_sheet(sheet);
            }
        }
    });
    if let Some(sheet) = app.ui.mobile.sheet {
        let panel = if end { egui::Panel::right("mobile-side-panel") } else { egui::Panel::left("mobile-side-panel") };
        panel.resizable(false).exact_size(SIDE_PANEL).frame(egui::Frame::NONE.fill(t.dock).inner_margin(egui::Margin::same(8))).show(ui, |ui| {
            super::sheet_body(app, ui, t, sheet);
        });
    }
}

/// Whether `route` is the editor: Home's chrome button (in the reused options bar) returns Home.
pub fn follow_home_button(app: &mut PhotocraftApp) {
    if app.ui.mobile.route == Route::Editor && app.ui.chrome.home.take().is_some() {
        app.ui.mobile.go(Route::Home);
    }
}

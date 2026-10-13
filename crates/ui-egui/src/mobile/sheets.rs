//! The Editor's sheets, each the desktop panel it stands for (`panels::dock_body` and friends) in
//! a touch frame: Layers (Layers | Channels | Paths), Brush (Brush Settings | Brushes), Curves
//! (the Properties panel of a Curves adjustment layer, non-modal: the image stays in view and
//! updates live), Color (Color | Swatches | Gradients | Patterns), History, Menus (every desktop
//! menu as an action sheet) and Export (Export As, Save, Save As).

use egui::{RichText, Sense, Stroke, vec2};
use serde_json::json;

use super::{Sheet, touch};
use crate::PhotocraftApp;
use crate::dock::Group;
use crate::theme::Tokens;

/// The sheet's panel under its title bar.
pub fn body(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens, sheet: Sheet) {
    match sheet {
        Sheet::Layers => tabbed(app, ui, t, Group::Layers),
        Sheet::Color => tabbed(app, ui, t, Group::Color),
        Sheet::History => fill(ui, |ui| crate::panels::dock_body(app, ui, Group::History, 0)),
        Sheet::Brush => brush(app, ui, t),
        Sheet::Curves => curves(app, ui),
        Sheet::Menus => menus(app, ui, t),
        Sheet::Export => export(app, ui, t),
    }
}

/// A panel given the rest of the sheet; panels that don't scroll themselves scroll here.
fn fill(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = ui.available_size();
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_size(size);
        add(ui);
    });
}

/// A panel laid out at the desktop's fixed width, scrolling sideways too on a narrow phone.
fn scrolled_wide(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]).show(ui, add);
}

fn scrolled(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// Pill tabs, as the desktop panel groups' (and the wireframe's segmented control).
fn tabs(ui: &mut egui::Ui, t: &Tokens, names: &[&str], current: &mut usize) -> bool {
    let mut changed = false;
    egui::ScrollArea::horizontal().id_salt(ui.id().with("tabs")).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, name) in names.iter().enumerate() {
                let on = *current == i;
                let label = crate::i18n::t(name);
                let g = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), if on { t.text } else { t.text_dim });
                let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 28.0, 40.0), Sense::click());
                if on {
                    ui.painter().rect_filled(rect, rect.height() / 2.0, t.row_selected);
                }
                ui.painter().galley(rect.center() - g.size() / 2.0, g, t.text);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
                if resp.clicked() && !on {
                    *current = i;
                    changed = true;
                }
            }
        });
    });
    ui.add_space(4.0);
    changed
}

/// A dock group's tabs over the chosen tab's panel. The tab is the dock's own, so the desktop
/// layout opens on the same one.
fn tabbed(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens, group: Group) {
    let names = group.tabs(t.pro);
    let mut tab = (*group.tab_mut(&mut app.ui.dock_tabs)).min(names.len().saturating_sub(1));
    tabs(ui, t, names, &mut tab);
    *group.tab_mut(&mut app.ui.dock_tabs) = tab;
    if group.scrolls_itself(tab) {
        fill(ui, |ui| crate::panels::dock_body(app, ui, group, tab));
    } else {
        scrolled(ui, group.key(), |ui| crate::panels::dock_body(app, ui, group, tab));
    }
}

/// Brush Settings and Brushes, the desktop's Brush Settings panel.
fn brush(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let mut tab = app.ui.brush_tab.min(1);
    tabs(ui, t, &["Brush Settings", "Brushes"], &mut tab);
    app.ui.brush_tab = tab;
    if tab == 1 {
        fill(ui, |ui| crate::brushes_tab::show(app, ui));
    } else {
        scrolled_wide(ui, "mobile-brush-settings", |ui| crate::brush_panel::settings_tab(app, ui));
    }
}

/// Is the active layer a Curves adjustment layer?
fn curves_layer_active(app: &PhotocraftApp) -> bool {
    app.session
        .active()
        .and_then(|st| st.active_layer.and_then(|id| st.doc.layer(id)))
        .is_some_and(|l| matches!(&l.content, photocraft_doc::LayerContent::Adjustment(photocraft_doc::Adjustment::Curves { .. })))
}

/// Curves: the Properties panel of the active Curves adjustment layer, as on the desktop. With
/// another layer active, one tap adds a Curves adjustment layer (Layer › New Adjustment Layer ›
/// Curves), and the other adjustments are a tap away below it.
fn curves(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if curves_layer_active(app) {
        scrolled(ui, "mobile-curves", |ui| crate::panels::properties_body(app, ui));
        return;
    }
    scrolled(ui, "mobile-adjustments", |ui| {
        if touch::action_card(ui, "sliders-horizontal", tl!("Curves"), ui.available_width(), true).clicked() {
            match app.run("layer.newAdjustmentLayer.curves", json!({})) {
                Ok(_) => app.ui.dock_tabs.properties = 0,
                Err(e) => app.ui.status = e,
            }
        }
        ui.add_space(12.0);
        crate::panels::dock_body(app, ui, Group::Properties, 1);
    });
}

/// The menus as an action sheet: search, the eleven menus as chips, then the chosen menu's items
/// in the desktop's order with their submenus and shortcuts. A tap runs the item and closes the
/// sheet.
fn menus(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let items = crate::menus::menu_items(app);
    let mut tops: Vec<&str> = Vec::new();
    for item in &items {
        if let Some(top) = item.path.first()
            && !tops.contains(&top.as_str())
        {
            tops.push(top);
        }
    }
    let state = &mut app.ui.mobile.menus;
    ui.add(egui::TextEdit::singleline(&mut state.search).hint_text(tl!("Search")).desired_width(ui.available_width()).margin(vec2(12.0, 10.0)));
    ui.add_space(4.0);
    let searching = !state.search.trim().is_empty();
    if !searching {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for top in &tops {
                let on = state.path.first().map(String::as_str) == Some(*top);
                if ui.add(egui::Button::selectable(on, crate::i18n::t(top)).min_size(vec2(0.0, 36.0)).corner_radius(18.0)).clicked() {
                    state.path = vec![top.to_string()];
                }
            }
        });
        ui.add_space(4.0);
    }
    if state.path.is_empty()
        && let Some(first) = tops.first()
    {
        state.path = vec![first.to_string()];
    }
    let path = state.path.clone();
    let query = state.search.trim().to_lowercase();
    let mut run = None;
    let mut open: Option<Vec<String>> = None;
    scrolled(ui, "mobile-menus", |ui| {
        if !searching
            && path.len() > 1
            && let Some((_, parent)) = path.split_last()
        {
            let parent = parent.to_vec();
            let label = format!("‹ {}", crate::i18n::t(path.last().map_or("", String::as_str)));
            if row(ui, t, &label, None, true, false).clicked() {
                open = Some(parent);
            }
        }
        let mut submenus: Vec<&str> = Vec::new();
        for item in &items {
            if searching {
                let label = crate::i18n::t(&item.label);
                if !(label.to_lowercase().contains(&query) || item.label.to_lowercase().contains(&query)) || item.id == "---" {
                    continue;
                }
                let trail = item.path.iter().map(|p| crate::i18n::t(p)).collect::<Vec<_>>().join(" › ");
                if row(ui, t, &format!("{label}    {trail}"), item.shortcut.as_deref(), item.enabled, false).clicked() {
                    run = Some(item.id.clone());
                }
                continue;
            }
            if !item.path.starts_with(&path) {
                continue;
            }
            if let Some(sub) = item.path.get(path.len()) {
                // A submenu: one row, which opens it.
                if !submenus.contains(&sub.as_str()) {
                    submenus.push(sub);
                    if row(ui, t, crate::i18n::t(sub), None, true, true).clicked() {
                        let mut deeper = path.clone();
                        deeper.push(sub.clone());
                        open = Some(deeper);
                    }
                }
            } else if item.id == "---" || item.label == "---" {
                widgets_separator(ui, t);
            } else if row(ui, t, crate::i18n::t(&item.label), item.shortcut.as_deref(), item.enabled, false).clicked() {
                run = Some(item.id.clone());
            }
        }
    });
    if let Some(p) = open {
        app.ui.mobile.menus.path = p;
    }
    if let Some(id) = run {
        app.ui.mobile.sheet = None;
        app.ui.mobile.menus.search.clear();
        let ctx = ui.ctx().clone();
        if let Err(e) = crate::menus::invoke(app, &ctx, &id, json!({})) {
            app.ui.status = e;
        }
    }
}

fn widgets_separator(ui: &mut egui::Ui, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.separator));
}

/// A 48 dp menu row: label, then the shortcut (for a hardware keyboard or DeX) or a submenu
/// chevron.
fn row(ui: &mut egui::Ui, t: &Tokens, label: &str, shortcut: Option<&str>, enabled: bool, submenu: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), touch::TARGET), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, t.radius, t.row_selected);
    }
    let color = if enabled { t.text } else { t.text_faint };
    let g = crate::tab_strip::elided(ui, label, egui::FontId::proportional(15.0), color, rect.width() - 120.0);
    ui.painter().galley(egui::pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, color);
    if submenu {
        crate::icons::paint(
            ui,
            egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.center().y), vec2(16.0, 16.0)),
            "chevron-right",
            16.0,
            t.text_dim,
        );
    } else if let Some(sc) = shortcut {
        ui.painter().text(egui::pos2(rect.right() - 12.0, rect.center().y), egui::Align2::RIGHT_CENTER, sc, crate::theme::mono(12.0), t.text_dim);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    resp
}

/// Export As (the desktop dialog's settings and preview, inline), then Save and Save As.
fn export(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &Tokens) {
    if app.ui.mobile.export.is_empty() {
        match crate::export_dialog::fields(app) {
            Ok(f) => app.ui.mobile.export = f,
            Err(e) => {
                ui.label(RichText::new(e).color(t.text_dim));
                return;
            }
        }
    }
    let mut f = std::mem::take(&mut app.ui.mobile.export);
    let mut export = false;
    let mut command = None;
    ui.horizontal(|ui| {
        export = touch::pill(ui, tl!("Export")).clicked();
        if ui.add(egui::Button::new(tl!("Save")).min_size(vec2(0.0, 40.0))).clicked() {
            command = Some("file.save");
        }
        if ui.add(egui::Button::new(tl!("Save As…")).min_size(vec2(0.0, 40.0))).clicked() {
            command = Some("file.saveAs");
        }
    });
    ui.add_space(6.0);
    scrolled_wide(ui, "mobile-export", |ui| crate::export_dialog::body(app, ui, &mut f));
    if export {
        // Asks where to save (on Android, the system's Save to…), then writes the file.
        if let Err(e) = crate::export_dialog::confirm(app, &f) {
            app.ui.status = e;
        }
        app.ui.mobile.sheet = None;
    } else if let Some(id) = command {
        app.ui.mobile.sheet = None;
        let ctx = ui.ctx().clone();
        if let Err(e) = crate::menus::invoke(app, &ctx, id, json!({})) {
            app.ui.status = e;
        }
    } else if app.ui.mobile.sheet == Some(Sheet::Export) {
        app.ui.mobile.export = f;
    }
}

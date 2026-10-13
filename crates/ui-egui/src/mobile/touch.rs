//! Touch-sized building blocks for the phone screens: 48 dp targets (44 dp in dense rows), the
//! theme's tokens, egui's own widgets underneath.

use egui::{Color32, Response, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::theme::Tokens;

/// Smallest touch target (dp).
pub const TARGET: f32 = 48.0;
/// Height of a field or a dense row (dp).
pub const FIELD: f32 = 44.0;
/// Height of the top bar.
pub const BAR: f32 = 56.0;

/// Enlarge egui's widgets in `ui` to touch size: buttons, combo boxes and their popups.
pub fn style(ui: &mut Ui) {
    let s = ui.spacing_mut();
    s.interact_size.y = FIELD;
    s.button_padding = vec2(12.0, 10.0);
    s.item_spacing = vec2(8.0, 8.0);
    s.combo_height = 360.0;
}

/// A square icon button with an accessible label (also its tooltip-free name for tests and
/// screen readers).
pub fn icon_button(ui: &mut Ui, icon: &str, label: &str, on: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(TARGET, TARGET), Sense::click());
    let fill = if on {
        t.row_selected
    } else if resp.is_pointer_button_down_on() {
        t.pressed
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect.shrink(2.0), t.radius, fill);
    crate::icons::paint(ui, rect, icon, 22.0, if on { t.text } else { t.icon });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// A filled, rounded button (the accent pill: Create).
pub fn pill(ui: &mut Ui, label: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), crate::theme::semibold(14.0), t.primary_text);
    let size = vec2(galley.size().x + 36.0, 40.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if resp.is_pointer_button_down_on() { t.accent_border } else { t.primary_bg };
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, t.primary_text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// A big tappable card with an icon over its label (Home's New Document and Open…).
pub fn action_card(ui: &mut Ui, icon: &str, label: &str, width: f32, primary: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 96.0), Sense::click());
    let (fill, text) = if primary { (t.primary_bg, t.primary_text) } else { (t.chrome, t.text) };
    let fill = if resp.is_pointer_button_down_on() { fill.gamma_multiply(0.85) } else { fill };
    ui.painter().rect_filled(rect, t.radius_lg, fill);
    if !primary {
        ui.painter().rect_stroke(rect, t.radius_lg, Stroke::new(1.0, t.field_border), StrokeKind::Inside);
    }
    let pad = 14.0;
    crate::icons::paint(ui, egui::Rect::from_min_size(rect.min + vec2(pad, pad), vec2(26.0, 26.0)), icon, 26.0, text);
    let galley = ui.painter().layout(label.to_owned(), crate::theme::semibold(15.0), text, width - 2.0 * pad);
    ui.painter().galley(egui::pos2(rect.left() + pad, rect.bottom() - pad - galley.size().y), galley, text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// A field's caption above it.
pub fn caption(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text).size(13.0).color(t.text_dim));
}

/// A section heading (uppercase, dim), as the desktop dialogs' group titles.
pub fn heading(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text.to_uppercase()).size(12.0).strong().color(t.text_dim));
}

/// A single-line text field, `FIELD` tall.
pub fn text(ui: &mut Ui, value: &mut String, width: f32) -> Response {
    ui.add_sized([width, FIELD], egui::TextEdit::singleline(value).vertical_align(egui::Align::Center).margin(vec2(12.0, 4.0)))
}

/// A number field, `FIELD` tall: tap to type, drag to scrub.
pub fn number(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, decimals: usize, suffix: &str, width: f32) -> Response {
    let field = egui::DragValue::new(value).range(range).max_decimals(decimals).suffix(suffix).speed(1.0);
    ui.add_sized([width, FIELD], field)
}

/// A drop-down of `options` (value, English label; translated here), `FIELD` tall. True when the
/// choice changed.
pub fn choice<T: PartialEq + Clone>(ui: &mut Ui, id: &str, current: &mut T, options: &[(T, &str)], width: f32) -> bool {
    let shown = options.iter().find(|(v, _)| v == current).map_or("—", |(_, l)| crate::i18n::t(l));
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).width(width).selected_text(shown).show_ui(ui, |ui| {
        for (v, label) in options {
            if ui.add_sized([width - 16.0, FIELD], egui::Button::selectable(v == current, crate::i18n::t(label))).clicked() && v != current {
                *current = v.clone();
                changed = true;
            }
        }
    });
    changed
}

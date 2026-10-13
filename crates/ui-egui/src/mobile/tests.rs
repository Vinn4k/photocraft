//! The phone router: layout mode, routes, sheets, Back, and driving them over the control channel.

use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use super::*;
use crate::control::{ControlRequest, Outcome, handle};

const PORTRAIT: egui::Vec2 = egui::vec2(412.0, 892.0);
const LANDSCAPE: egui::Vec2 = egui::vec2(892.0, 412.0);

fn call(app: &mut PhotocraftApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
    let (req, _rx) = ControlRequest::new(method, params);
    match handle(app, ctx, &req) {
        Outcome::Done(v) => v,
        _ => panic!("{method}: expected an immediate reply"),
    }
}

fn app_with_document() -> PhotocraftApp {
    let mut s = photocraft_engine::Session::new();
    s.execute("file.new", json!({"width": 64, "height": 48})).unwrap();
    PhotocraftApp::new(s, crate::Services::default())
}

fn editor() -> MobileState {
    let mut m = MobileState::default();
    m.go(Route::NewDocument);
    m.replace(Route::Editor);
    m
}

#[test]
fn layout_mode_follows_compact_width_or_height() {
    assert_eq!(LayoutMode::for_size(PORTRAIT), LayoutMode::Phone);
    assert_eq!(LayoutMode::for_size(LANDSCAPE), LayoutMode::Phone, "a phone held sideways is compact in height");
    // The desktop app's smallest window and every bigger one keep the desktop layout.
    assert_eq!(LayoutMode::for_size(egui::vec2(760.0, 480.0)), LayoutMode::Desktop);
    assert_eq!(LayoutMode::for_size(egui::vec2(1440.0, 900.0)), LayoutMode::Desktop);
    assert_eq!(LayoutMode::for_size(egui::vec2(599.0, 900.0)), LayoutMode::Phone);
    assert_eq!(LayoutMode::for_size(egui::vec2(900.0, 479.0)), LayoutMode::Phone);
    assert_eq!(Orientation::for_size(PORTRAIT), Orientation::Portrait);
    assert_eq!(Orientation::for_size(LANDSCAPE), Orientation::Landscape);
}

#[test]
fn the_desktop_app_keeps_its_layout_unless_forced() {
    // Tests build for the desktop: a small window is no reason to change its layout.
    assert_eq!(LayoutPref::default(), LayoutPref::Desktop);
    assert_eq!(LayoutPref::Desktop.resolve(PORTRAIT), LayoutMode::Desktop);
    assert_eq!(LayoutPref::Auto.resolve(PORTRAIT), LayoutMode::Phone);
    assert_eq!(LayoutPref::Auto.resolve(egui::vec2(1440.0, 900.0)), LayoutMode::Desktop);
    assert_eq!(LayoutPref::Phone.resolve(egui::vec2(1440.0, 900.0)), LayoutMode::Phone);
}

#[test]
fn ids_round_trip() {
    for r in Route::ALL {
        assert_eq!(Route::from_id(r.id()), Some(r));
        assert_eq!(serde_json::to_value(r).unwrap(), json!(r.id()), "serde and ids agree");
    }
    for s in Sheet::ALL {
        assert_eq!(Sheet::from_id(s.id()), Some(s));
        assert_eq!(serde_json::to_value(s).unwrap(), json!(s.id()));
    }
    for p in LayoutPref::ALL {
        assert_eq!(LayoutPref::from_id(p.id()), Some(p));
    }
    assert_eq!(Route::from_id("Editor"), None);
}

#[test]
fn back_walks_the_stack_to_home() {
    let mut m = MobileState::default();
    assert!(!m.can_go_back());
    assert!(!m.back(), "Home with nothing open leaves Back to the system");
    m.go(Route::NewDocument);
    m.go(Route::Editor);
    assert_eq!(m.history, [Route::Home, Route::NewDocument]);
    assert!(m.back());
    assert_eq!(m.route, Route::NewDocument);
    assert!(m.back());
    assert_eq!(m.route, Route::Home);
    assert!(!m.back());
}

#[test]
fn create_replaces_new_document_so_back_returns_home() {
    let mut m = editor();
    assert_eq!(m.route, Route::Editor);
    assert_eq!(m.history, [Route::Home]);
    assert!(m.back());
    assert_eq!(m.route, Route::Home);
}

#[test]
fn going_to_a_route_on_the_stack_unwinds_to_it() {
    let mut m = MobileState::default();
    m.go(Route::NewDocument);
    m.go(Route::Editor);
    m.go(Route::NewDocument);
    assert_eq!(m.history, [Route::Home], "no New Document → Editor → New Document loop");
    m.go(Route::Editor);
    m.go(Route::Home);
    assert!(m.history.is_empty(), "Home is the root");
    m.go(Route::Home);
    assert!(m.history.is_empty(), "going where we are is a no-op");
}

#[test]
fn one_sheet_at_a_time_over_the_editor_only() {
    let mut m = MobileState::default();
    assert!(m.open_sheet(Sheet::Layers).is_err(), "sheets belong to the editor");
    assert_eq!(m.sheet, None);
    let mut m = editor();
    m.open_sheet(Sheet::Layers).unwrap();
    m.open_sheet(Sheet::Brush).unwrap();
    assert_eq!(m.sheet, Some(Sheet::Brush), "opening one closes the other");
    m.toggle_sheet(Sheet::Brush).unwrap();
    assert_eq!(m.sheet, None, "its button again closes it");
    m.toggle_sheet(Sheet::Curves).unwrap();
    assert_eq!(m.sheet, Some(Sheet::Curves));
    // Back closes the sheet first, then leaves the editor.
    assert!(m.back());
    assert_eq!((m.route, m.sheet), (Route::Editor, None));
    m.open_sheet(Sheet::Export).unwrap();
    m.go(Route::Home);
    assert_eq!(m.sheet, None, "leaving the editor closes its sheet");
}

#[test]
fn every_sheet_opens() {
    let mut m = editor();
    for s in Sheet::ALL {
        m.open_sheet(s).unwrap();
        assert_eq!(m.sheet, Some(s));
    }
}

#[test]
fn closing_the_last_document_returns_home() {
    let mut m = editor();
    m.open_sheet(Sheet::Color).unwrap();
    m.sync(true);
    assert_eq!(m.route, Route::Editor);
    m.sync(false);
    assert_eq!((m.route, m.sheet), (Route::Home, None));
    assert!(m.history.is_empty());
}

#[test]
fn apply_validates_everything_before_changing_anything() {
    let mut m = MobileState::default();
    let before = m.clone();
    for bad in [
        json!("phone"),
        json!({"layout": "tablet"}),
        json!({"layout": 3}),
        json!({"route": "gallery"}),
        json!({"sheet": "swatches"}),
        json!({"sheet": 1}),
        json!({"back": "yes"}),
        json!({"route": "newDocument", "colour": true}),
        // Valid route, but the sheet needs the editor: nothing applies.
        json!({"layout": "phone", "route": "newDocument", "sheet": "layers"}),
        json!({"back": true}),
    ] {
        assert!(m.apply(&bad, true).is_err(), "{bad}");
        assert_eq!(m, before, "{bad}: a rejected call changes nothing");
    }
    assert!(m.apply(&json!({"route": "editor"}), false).unwrap_err().contains("open document"));
    m.apply(&Value::Null, true).unwrap();
    assert_eq!(m, before, "null is a no-op");
    m.apply(&json!({"layout": "auto", "route": "editor", "sheet": "menus"}), true).unwrap();
    assert_eq!((m.layout, m.route, m.sheet), (LayoutPref::Auto, Route::Editor, Some(Sheet::Menus)));
    m.apply(&json!({"sheet": null}), true).unwrap();
    assert_eq!(m.sheet, None);
    m.apply(&json!({"back": true}), true).unwrap();
    assert_eq!(m.route, Route::Home);
}

#[test]
fn ui_set_navigates_and_ui_inspect_reports_it() {
    let mut app = app_with_document();
    let ctx = egui::Context::default();
    let r = call(&mut app, &ctx, "ui.set", json!({"mobile": {"layout": "phone", "route": "newDocument"}}));
    assert_eq!(r["ok"], true, "{r}");
    let r = call(&mut app, &ctx, "ui.set", json!({"mobile": {"route": "editor", "sheet": "layers"}}));
    assert_eq!(r["ok"], true, "{r}");
    let m = &call(&mut app, &ctx, "ui.inspect", json!({}))["result"]["mobile"];
    assert_eq!(m["layout"], "phone");
    assert_eq!(m["mode"], "phone");
    assert_eq!(m["route"], "editor");
    assert_eq!(m["sheet"], "layers");
    assert_eq!(m["history"], json!(["home", "newDocument"]));
    assert_eq!(m["canGoBack"], true);
    // A bad mobile value rejects the whole call, other fields included.
    let r = call(&mut app, &ctx, "ui.set", json!({"tool": "eraser", "mobile": {"sheet": "nope"}}));
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("mobile.sheet"), "{r}");
    assert_eq!(app.ui.tool, crate::state::Tool::Brush);
    assert_eq!(app.ui.mobile.sheet, Some(Sheet::Layers));
    for _ in 0..3 {
        assert_eq!(call(&mut app, &ctx, "ui.set", json!({"mobile": {"back": true}}))["ok"], true);
    }
    assert_eq!(app.ui.mobile.route, Route::Home);
    assert_eq!(call(&mut app, &ctx, "ui.set", json!({"mobile": {"back": true}}))["ok"], false, "nothing left to go back to");
}

fn harness(size: egui::Vec2, layout: LayoutPref, document: bool) -> egui_kittest::Harness<'static, PhotocraftApp> {
    let mut h = egui_kittest::Harness::builder().with_size(size).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = if document { app_with_document() } else { PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default()) };
        app.ui.mobile.layout = layout;
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn a_phone_sized_window_draws_the_router_and_a_desktop_one_does_not() {
    let h = harness(PORTRAIT, LayoutPref::Auto, false);
    assert!(h.query_by_label("New Document").is_some(), "Home's New Document");
    let h = harness(egui::vec2(1200.0, 800.0), LayoutPref::Auto, false);
    assert!(h.query_by_label("Create a new layer").is_none() && h.query_by_label("Back").is_none(), "desktop chrome, no router");
    // The desktop default keeps the desktop chrome in a phone-sized window.
    let h = harness(PORTRAIT, LayoutPref::Desktop, true);
    assert!(h.query_by_label("Back").is_none());
}

#[test]
fn taps_navigate_from_home_to_a_sheet_and_back_key_returns() {
    let mut h = harness(PORTRAIT, LayoutPref::Auto, false);
    h.get_by_label("New Document").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::NewDocument);
    h.get_by_label("Create").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::Editor);
    assert!(h.state().session.active().is_some(), "Create made a document");
    h.get_by_label("Layers").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.sheet, Some(Sheet::Layers));
    // Android's Back (egui's BrowserBack) closes the sheet, then returns Home.
    for expected in [(Route::Editor, None), (Route::Home, None)] {
        h.key_press(egui::Key::BrowserBack);
        h.run_steps(4);
        assert_eq!((h.state().ui.mobile.route, h.state().ui.mobile.sheet), expected);
    }
}

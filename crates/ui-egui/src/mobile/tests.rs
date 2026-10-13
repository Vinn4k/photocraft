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
    m.sync(1);
    assert_eq!(m.route, Route::Editor);
    m.sync(0);
    assert_eq!((m.route, m.sheet), (Route::Home, None));
    assert!(m.history.is_empty());
}

#[test]
fn a_document_opened_from_home_or_new_document_shows_the_editor() {
    let mut m = MobileState::default();
    m.sync(0);
    m.sync(1);
    assert_eq!(m.route, Route::Editor, "an open from Home");
    assert!(m.back());
    assert_eq!(m.route, Route::Home);
    m.sync(1);
    assert_eq!(m.route, Route::Home, "the same documents: Home stays");
    m.go(Route::NewDocument);
    m.new_document.insert("name".into(), json!("Poster"));
    m.sync(2);
    assert_eq!((m.route, m.history.as_slice()), (Route::Editor, [Route::Home].as_slice()), "Back skips New Document");
    assert!(m.new_document.is_empty(), "leaving New Document drops its fields");
}

#[test]
fn ui_set_edits_the_new_document_fields_on_that_screen_only() {
    let mut m = MobileState::default();
    assert!(m.apply(&json!({"newDocument": {"width": 640}}), false).unwrap_err().contains("route newDocument"));
    assert!(m.apply(&json!({"route": "newDocument", "newDocument": 3}), false).is_err());
    assert_eq!(m.route, Route::Home, "rejected: nothing applied");
    m.apply(&json!({"route": "newDocument", "newDocument": {"width": 640, "height": 480}}), false).unwrap();
    assert_eq!((m.new_document["width"].clone(), m.new_document["height"].clone()), (json!(640), json!(480)));
    assert_eq!(m.inspect(PORTRAIT)["newDocument"]["width"], 640);
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
fn new_document_creates_the_picked_preset() {
    let mut h = harness(PORTRAIT, LayoutPref::Auto, false);
    h.get_by_label("New Document").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::NewDocument);
    assert!(!h.state().ui.mobile.new_document.is_empty(), "the dialog's fields, filled on arrival");
    h.get_by_label("Web").click();
    h.run_steps(4);
    // Small enough for the test renderer's 2048 px textures.
    h.get_by_label("Web Minimum").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.new_document["width"], json!(1024));
    h.get_by_label("Create").click();
    h.run_steps(4);
    let st = h.state().session.active().expect("Create made a document");
    assert_eq!((st.doc.size.width, st.doc.size.height), (1024, 768));
    assert_eq!(h.state().ui.mobile.route, Route::Editor);
    assert_eq!(h.state().session.presets.recent_documents.first().map(|s| (s.width, s.height)), Some((1024, 768)), "listed under Recent, as the dialog does");
    h.key_press(egui::Key::BrowserBack);
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::Home, "Back from the new document returns Home");
}

#[test]
fn new_document_close_returns_home_without_creating() {
    let mut h = harness(PORTRAIT, LayoutPref::Auto, false);
    h.get_by_label("New Document").click();
    h.run_steps(4);
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::Home);
    assert!(h.state().session.active().is_none());
    assert!(h.state().ui.mobile.new_document.is_empty());
}

#[test]
fn home_lists_recent_files_and_an_unreadable_one_reports_instead_of_opening() {
    let mut h = egui_kittest::Harness::builder().with_size(PORTRAIT).with_max_steps(64).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.ui.mobile.layout = LayoutPref::Auto;
        // The list lives in the preferences; `ui.recent_files` mirrors it every frame.
        app.push_recent("/nonexistent/photocraft-test/poster.psd");
        app.push_recent("/nonexistent/photocraft-test/moon.png");
        app
    });
    h.run_steps(4);
    assert!(h.query_by_label("moon.png").is_some());
    h.get_by_label("poster.psd").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::Home, "nothing opened");
    assert!(h.state().session.active().is_none());
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

fn editor_harness(size: egui::Vec2) -> egui_kittest::Harness<'static, PhotocraftApp> {
    let mut h = harness(size, LayoutPref::Auto, true);
    // The document was open at launch: the router shows it.
    assert_eq!(h.state().ui.mobile.route, Route::Editor);
    h.run_steps(2);
    h
}

#[test]
fn portrait_editor_bars_open_their_sheets() {
    let mut h = editor_harness(PORTRAIT);
    for (label, sheet) in [
        ("Layers", Sheet::Layers),
        ("Adjustments", Sheet::Curves),
        ("Properties", Sheet::Brush),
        ("History", Sheet::History),
        ("Color", Sheet::Color),
        ("Menus", Sheet::Menus),
        ("Export", Sheet::Export),
        ("Set foreground color", Sheet::Color),
    ] {
        h.state_mut().ui.mobile.sheet = None;
        h.run_steps(2);
        h.get_by_label(label).click();
        h.run_steps(4);
        assert_eq!(h.state().ui.mobile.sheet, Some(sheet), "{label}");
    }
    // The open sheet's Close, then Back to Home.
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.sheet, None);
    h.get_by_label("Back").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.route, Route::Home);
    assert!(h.state().session.active().is_some(), "the document stays open");
}

#[test]
fn tool_bar_and_all_tools_pick_tools() {
    let mut h = editor_harness(PORTRAIT);
    h.get_by_label("Eraser Tool").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.tool, crate::state::Tool::Eraser);
    h.get_by_label("Tools").click();
    h.run_steps(4);
    assert!(h.state().ui.mobile.all_tools);
    h.get_by_label("Gradient Tool").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.tool, crate::state::Tool::Gradient);
    assert!(!h.state().ui.mobile.all_tools, "picking a tool closes the grid");
}

#[test]
fn undo_in_the_top_bar_undoes() {
    let mut h = editor_harness(PORTRAIT);
    h.state_mut().run("layer.new.layer", json!({})).unwrap();
    let layers = |h: &egui_kittest::Harness<'static, PhotocraftApp>| h.state().session.active().unwrap().doc.layers.len();
    assert_eq!(layers(&h), 2);
    h.run_steps(2);
    h.get_by_label("Undo").click();
    h.run_steps(4);
    assert_eq!(layers(&h), 1);
    h.get_by_label("Redo").click();
    h.run_steps(4);
    assert_eq!(layers(&h), 2);
}

#[test]
fn the_brush_rail_leaves_the_brush_alone_until_moved() {
    let mut h = editor_harness(PORTRAIT);
    let before = h.state().session.tools.brush.clone();
    h.run_steps(6);
    assert_eq!(h.state().session.tools.brush, before, "drawing the rail writes nothing (opacity is 0..1, the rail 1..100%)");
}

#[test]
fn landscape_editor_has_a_tool_column_and_a_side_panel() {
    let mut h = editor_harness(LANDSCAPE);
    assert!(h.query_by_label("Eraser Tool").is_some(), "tools down the side");
    h.get_by_label("Layers").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.sheet, Some(Sheet::Layers));
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.sheet, None, "the panel collapses to its strip");
}

#[test]
fn on_screen_keys_toggle_and_ride_on_the_input() {
    let mut h = editor_harness(PORTRAIT);
    h.get_by_label("Alt").click();
    h.run_steps(2);
    assert!(h.state().ui.mobile.modifiers.alt);
    let ctx = h.ctx.clone();
    let tap =
        egui::Event::PointerButton { pos: egui::pos2(200.0, 400.0), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE };
    let mut raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, PORTRAIT)), events: vec![tap.clone()], ..Default::default() };
    crate::mobile::hold_modifiers(h.state_mut(), &ctx, &mut raw);
    assert!(matches!(raw.events.first(), Some(egui::Event::ModifiersChanged(m)) if m.alt && !m.shift));
    assert!(matches!(raw.events.get(1), Some(egui::Event::PointerButton { modifiers, .. }) if modifiers.alt), "the tap is an Alt-click");
    // Released: the next frame takes Alt off again.
    h.get_by_label("Alt").click();
    h.run_steps(2);
    let mut raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, PORTRAIT)), events: vec![tap], ..Default::default() };
    crate::mobile::hold_modifiers(h.state_mut(), &ctx, &mut raw);
    assert!(matches!(raw.events.first(), Some(egui::Event::ModifiersChanged(m)) if !m.alt));
    assert!(matches!(raw.events.get(1), Some(egui::Event::PointerButton { modifiers, .. }) if !modifiers.alt));
    // Leaving the editor drops held keys.
    h.state_mut().ui.mobile.modifiers.shift = true;
    h.state_mut().ui.mobile.go(Route::Home);
    h.run_steps(2);
    assert!(!h.state().ui.mobile.modifiers.any());
}

#[test]
fn ui_set_holds_the_on_screen_keys() {
    let mut m = editor();
    m.apply(&json!({"modifiers": {"shift": true, "ctrl": true}}), true).unwrap();
    assert_eq!(m.modifiers, Modifiers { shift: true, alt: false, ctrl: true });
    for bad in [json!({"modifiers": {"meta": true}}), json!({"modifiers": {"shift": 1}}), json!({"modifiers": true})] {
        assert!(m.apply(&bad, true).is_err(), "{bad}");
    }
    assert_eq!(m.inspect(PORTRAIT)["modifiers"]["shift"], true);
}

/// The highest of the buttons labelled `label` (a sheet's, above the editor's bars).
fn topmost<'a>(h: &'a egui_kittest::Harness<'static, PhotocraftApp>, label: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, label)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .unwrap_or_else(|| panic!("no {label}"))
}

#[test]
fn every_sheet_draws_its_panel_in_both_orientations() {
    for size in [PORTRAIT, LANDSCAPE] {
        let mut h = editor_harness(size);
        for (sheet, shows) in [
            (Sheet::Layers, "Channels"),
            (Sheet::Brush, "Brushes"),
            (Sheet::Color, "Swatches"),
            (Sheet::Menus, "Window"),
            (Sheet::Export, "Save As…"),
            (Sheet::History, "History"),
            (Sheet::Curves, "Curves"),
        ] {
            h.state_mut().ui.mobile.sheet = Some(sheet);
            h.run_steps(4);
            assert!(h.query_all_by_label(shows).next().is_some(), "{size:?} {sheet:?}: {shows}");
        }
    }
}

#[test]
fn layers_sheet_tabs_are_the_docks() {
    let mut h = editor_harness(PORTRAIT);
    h.state_mut().ui.mobile.sheet = Some(Sheet::Layers);
    h.run_steps(4);
    h.get_by_label("Channels").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.dock_tabs.layers, 1, "the dock's tab: the desktop opens on it too");
}

#[test]
fn curves_sheet_adds_a_curves_layer_and_shows_its_properties() {
    let mut h = editor_harness(PORTRAIT);
    h.state_mut().ui.mobile.sheet = Some(Sheet::Curves);
    h.run_steps(4);
    // The card, above the adjustments grid's own Curves button.
    topmost(&h, "Curves").click();
    h.run_steps(6);
    let st = h.state().session.active().unwrap();
    let layer = st.active_layer.and_then(|id| st.doc.layer(id)).expect("an active layer");
    assert!(matches!(&layer.content, photocraft_doc::LayerContent::Adjustment(photocraft_doc::Adjustment::Curves { .. })), "a Curves adjustment layer");
    assert_eq!(h.state().ui.mobile.sheet, Some(Sheet::Curves), "non-modal: the sheet stays, now on its properties");
}

#[test]
fn menus_sheet_opens_submenus_and_runs_items() {
    let mut h = editor_harness(PORTRAIT);
    h.state_mut().ui.mobile.sheet = Some(Sheet::Menus);
    h.state_mut().ui.mobile.menus.path = vec!["Image".into()];
    h.run_steps(4);
    // The menu row, above the panel bar's Adjustments.
    topmost(&h, "Adjustments").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.menus.path, ["Image", "Adjustments"]);
    h.get_by_label("‹ Adjustments").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.menus.path, ["Image"]);
    // Search finds Select › All anywhere; a tap runs it and closes the sheet.
    h.state_mut().ui.mobile.menus.search = "deselect".into();
    h.run_steps(4);
    assert!(h.query_by_label("Select All").is_none());
    h.state_mut().ui.mobile.menus.search.clear();
    h.state_mut().ui.mobile.menus.path = vec!["Select".into()];
    h.run_steps(4);
    h.get_by_label("All").click();
    h.run_steps(4);
    assert!(h.state().session.active().unwrap().doc.selection.is_some(), "Select › All ran");
    assert_eq!(h.state().ui.mobile.sheet, None);
}

#[test]
fn export_sheet_holds_export_as_fields_while_open() {
    let mut h = editor_harness(PORTRAIT);
    h.state_mut().ui.mobile.sheet = Some(Sheet::Export);
    h.run_steps(4);
    assert_eq!(h.state().ui.mobile.export.get("format"), Some(&json!("png")), "the Export As dialog's fields");
    assert_eq!(h.state().ui.mobile.export.get("__w"), Some(&json!(64)));
    h.get_by_label("Close").click();
    h.run_steps(4);
    assert!(h.state().ui.mobile.export.is_empty(), "dropped when the sheet closes");
}

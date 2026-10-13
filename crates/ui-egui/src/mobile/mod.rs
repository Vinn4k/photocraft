//! The phone layout (Android port, `docs/android-mobile.md`): a router of full-screen routes
//! (Home, New Document, Editor) and the Editor's bottom sheets (Layers, Brush, Curves, Color,
//! Menus, Export), one sheet at a time, with a back stack.
//!
//! The layout mode follows the window size ([`LayoutMode::for_size`]); the desktop app keeps its
//! layout unless the mode is forced ([`LayoutPref`]). All of this is plain data in
//! [`crate::state::UiState::mobile`], so `ui.set {"mobile": {…}}` navigates and `ui.inspect`
//! reports where the app is ([`MobileState::inspect`]).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::PhotocraftApp;

mod home;
mod new_document;
#[cfg(test)]
mod tests;
pub mod touch;

/// Below this width (points, which are dp on Android) the window is a phone held upright:
/// Android's compact width class.
pub const PHONE_MAX_WIDTH: f32 = 600.0;
/// Below this height the window is a phone held sideways: Android's compact height class. The
/// desktop app's smallest window (760 × 480) is above both limits.
pub const PHONE_MAX_HEIGHT: f32 = 480.0;

/// Which layout the shell draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutMode {
    /// Menus, options bar, toolbar and dock: the desktop shell.
    Desktop,
    /// The phone router ([`Route`], [`Sheet`]).
    Phone,
}

impl LayoutMode {
    /// The layout for a window of `size` points: a phone when either side is compact.
    pub fn for_size(size: egui::Vec2) -> Self {
        if size.x < PHONE_MAX_WIDTH || size.y < PHONE_MAX_HEIGHT { Self::Phone } else { Self::Desktop }
    }
}

/// How the layout mode is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutPref {
    /// By the window size ([`LayoutMode::for_size`]).
    Auto,
    Desktop,
    Phone,
}

impl LayoutPref {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Desktop, Self::Phone];

    /// Android and the browser follow the screen. The desktop app keeps its layout whatever the
    /// window size, unless the control channel asks for another.
    pub fn platform_default() -> Self {
        if cfg!(any(target_os = "android", target_arch = "wasm32")) { Self::Auto } else { Self::Desktop }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Desktop => "desktop",
            Self::Phone => "phone",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    pub fn resolve(self, size: egui::Vec2) -> LayoutMode {
        match self {
            Self::Auto => LayoutMode::for_size(size),
            Self::Desktop => LayoutMode::Desktop,
            Self::Phone => LayoutMode::Phone,
        }
    }
}

impl Default for LayoutPref {
    fn default() -> Self {
        Self::platform_default()
    }
}

/// Portrait is the main phone layout; landscape is its drawn variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn for_size(size: egui::Vec2) -> Self {
        if size.x > size.y { Self::Landscape } else { Self::Portrait }
    }
}

/// A full-screen phone screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    /// Start screen: New Document, Open…, recent files.
    #[default]
    Home,
    /// Presets and the new document's details.
    NewDocument,
    /// The canvas and its bars; needs an open document.
    Editor,
}

impl Route {
    pub const ALL: [Self; 3] = [Self::Home, Self::NewDocument, Self::Editor];

    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::NewDocument => "newDocument",
            Self::Editor => "editor",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// The English title (translated where it is drawn).
    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::NewDocument => "New Document",
            Self::Editor => "Editor",
        }
    }
}

/// A sheet over the Editor. Only one is open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sheet {
    Layers,
    Brush,
    /// Adjustments: Curves, non-modal (the image stays visible).
    Curves,
    Color,
    /// The desktop menus as an action sheet.
    Menus,
    /// Export As and Save.
    Export,
}

impl Sheet {
    pub const ALL: [Self; 6] = [Self::Layers, Self::Brush, Self::Curves, Self::Color, Self::Menus, Self::Export];

    pub fn id(self) -> &'static str {
        match self {
            Self::Layers => "layers",
            Self::Brush => "brush",
            Self::Curves => "curves",
            Self::Color => "color",
            Self::Menus => "menus",
            Self::Export => "export",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The English title (translated where it is drawn).
    pub fn title(self) -> &'static str {
        match self {
            Self::Layers => "Layers",
            Self::Brush => "Brush",
            Self::Curves => "Curves",
            Self::Color => "Color",
            Self::Menus => "Menus",
            Self::Export => "Export",
        }
    }
}

/// Where the phone layout is: the route, its open sheet and the routes Back returns to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MobileState {
    pub layout: LayoutPref,
    pub route: Route,
    pub sheet: Option<Sheet>,
    /// Routes Back returns to, oldest first. Home is the root, so it is never above another route.
    pub history: Vec<Route>,
    /// The New Document screen's fields, as the desktop dialog's (`new_doc_ui`): filled when the
    /// screen opens, dropped when it is left.
    pub new_document: serde_json::Map<String, Value>,
    /// Open documents last frame: one more means a document was opened or created.
    #[serde(skip)]
    pub seen_documents: usize,
}

impl MobileState {
    /// Show `route`. Home starts over; a route already on the stack unwinds to it (Home → New
    /// Document → Editor → New Document leaves Back leading to Home, not round in a circle).
    /// Leaving a route closes its sheet.
    pub fn go(&mut self, route: Route) {
        if route == self.route {
            return;
        }
        self.sheet = None;
        if route == Route::Home {
            self.history.clear();
        } else if let Some(at) = self.history.iter().position(|r| *r == route) {
            self.history.truncate(at);
        } else {
            self.history.push(self.route);
        }
        self.route = route;
    }

    /// Show `route` in place of the current one, which Back then skips (New Document's Create
    /// opens the Editor, and Back from there returns Home).
    pub fn replace(&mut self, route: Route) {
        self.sheet = None;
        if route == Route::Home {
            self.history.clear();
        } else if let Some(at) = self.history.iter().position(|r| *r == route) {
            self.history.truncate(at);
        }
        self.route = route;
    }

    /// Open `sheet` over the Editor, closing any other (one panel at a time on a phone).
    pub fn open_sheet(&mut self, sheet: Sheet) -> Result<(), String> {
        if self.route != Route::Editor {
            return Err(format!("the {} sheet opens over the editor (the route is {})", sheet.id(), self.route.id()));
        }
        self.sheet = Some(sheet);
        Ok(())
    }

    /// Open `sheet`, or close it when it is the one open (a panel button pressed twice).
    pub fn toggle_sheet(&mut self, sheet: Sheet) -> Result<(), String> {
        if self.sheet == Some(sheet) {
            self.sheet = None;
            Ok(())
        } else {
            self.open_sheet(sheet)
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.sheet.is_some() || self.route != Route::Home
    }

    /// Android's Back: close the sheet, else return to the previous route. `false` at Home with
    /// nothing open, where the system takes Back (and leaves the app).
    pub fn back(&mut self) -> bool {
        if self.sheet.take().is_some() {
            return true;
        }
        if self.route == Route::Home {
            return false;
        }
        self.route = self.history.pop().unwrap_or(Route::Home);
        true
    }

    /// Follow the open documents (`documents`, this frame): a document opened or created from
    /// Home or New Document shows the Editor (as the desktop Home screen gives way to it), and
    /// closing the last one returns Home, as the Editor needs a document.
    pub fn sync(&mut self, documents: usize) {
        if documents > self.seen_documents && self.route != Route::Editor {
            self.replace(Route::Editor);
        }
        self.seen_documents = documents;
        if self.route == Route::Editor && documents == 0 {
            self.sheet = None;
            self.history.clear();
            self.route = Route::Home;
        }
        if self.route != Route::NewDocument {
            self.new_document.clear();
        }
    }

    /// Apply `ui.set`'s `mobile` object: `{layout?, route?, sheet?, back?}`, in that order. Every
    /// key and value is checked first, so a rejected call changes nothing. `null` is a no-op;
    /// `sheet: null` closes the sheet.
    pub fn apply(&mut self, params: &Value, has_document: bool) -> Result<(), String> {
        let plan = Plan::parse(params)?;
        let mut next = self.clone();
        if let Some(layout) = plan.layout {
            next.layout = layout;
        }
        if let Some(route) = plan.route {
            if route == Route::Editor && !has_document {
                return Err("the editor needs an open document (file.new or app.open first)".into());
            }
            next.go(route);
        }
        match plan.sheet {
            Some(Some(sheet)) => next.open_sheet(sheet)?,
            Some(None) => next.sheet = None,
            None => {}
        }
        if let Some(fields) = plan.new_document {
            if next.route != Route::NewDocument {
                return Err("mobile.newDocument edits the New Document screen (route newDocument)".into());
            }
            next.new_document.extend(fields);
        }
        if plan.back && !next.back() {
            return Err("nothing to go back to (Home with no sheet open)".into());
        }
        *self = next;
        Ok(())
    }

    /// The state for `ui.inspect`, with the layout this window size resolves to.
    pub fn inspect(&self, size: egui::Vec2) -> Value {
        json!({
            "layout": self.layout.id(),
            "mode": self.layout.resolve(size),
            "orientation": Orientation::for_size(size),
            "route": self.route.id(),
            "sheet": self.sheet.map(Sheet::id),
            "history": self.history.iter().map(|r| r.id()).collect::<Vec<_>>(),
            "canGoBack": self.can_go_back(),
            "newDocument": self.new_document,
        })
    }
}

/// `ui.set`'s `mobile` object, validated.
#[derive(Default)]
struct Plan {
    layout: Option<LayoutPref>,
    route: Option<Route>,
    /// `Some(None)`: close the sheet.
    sheet: Option<Option<Sheet>>,
    back: bool,
    /// Fields merged into the New Document screen's (the desktop dialog's field names).
    new_document: Option<serde_json::Map<String, Value>>,
}

impl Plan {
    const KEYS: [&'static str; 5] = ["layout", "route", "sheet", "back", "newDocument"];

    fn parse(params: &Value) -> Result<Self, String> {
        let mut plan = Self::default();
        if params.is_null() {
            return Ok(plan);
        }
        let Some(o) = params.as_object() else { return Err("mobile must be an object {layout?, route?, sheet?, back?, newDocument?}".into()) };
        if let Some(key) = o.keys().find(|k| !Self::KEYS.contains(&k.as_str())) {
            return Err(format!("unknown mobile field `{key}` (fields: {})", Self::KEYS.join(", ")));
        }
        let ids = |names: &mut dyn Iterator<Item = &'static str>| names.collect::<Vec<_>>().join(", ");
        if let Some(v) = o.get("layout").filter(|v| !v.is_null()) {
            let id = v.as_str().unwrap_or_default();
            plan.layout =
                Some(LayoutPref::from_id(id).ok_or_else(|| format!("mobile.layout must be one of {}", ids(&mut LayoutPref::ALL.iter().map(|p| p.id()))))?);
        }
        if let Some(v) = o.get("route").filter(|v| !v.is_null()) {
            let id = v.as_str().unwrap_or_default();
            plan.route = Some(Route::from_id(id).ok_or_else(|| format!("mobile.route must be one of {}", ids(&mut Route::ALL.iter().map(|r| r.id()))))?);
        }
        if let Some(v) = o.get("sheet") {
            plan.sheet = Some(match v {
                Value::Null => None,
                v => Some(
                    v.as_str()
                        .and_then(Sheet::from_id)
                        .ok_or_else(|| format!("mobile.sheet must be null or one of {}", ids(&mut Sheet::ALL.iter().map(|s| s.id()))))?,
                ),
            });
        }
        if let Some(v) = o.get("back").filter(|v| !v.is_null()) {
            plan.back = v.as_bool().ok_or("mobile.back must be true or false")?;
        }
        if let Some(v) = o.get("newDocument").filter(|v| !v.is_null()) {
            plan.new_document = Some(v.as_object().cloned().ok_or("mobile.newDocument must be an object of New Document fields")?);
        }
        Ok(plan)
    }
}

/// The layout mode this frame, for a window of the context's size.
pub fn mode(app: &PhotocraftApp, ctx: &egui::Context) -> LayoutMode {
    app.ui.mobile.layout.resolve(ctx.content_rect().size())
}

/// Draw the phone layout: the route's screen and, over the Editor, its sheet.
pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    app.ui.mobile.sync(app.session.documents().len());
    // Android's Back arrives as the browser Back key (egui-winit maps it).
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) {
        app.ui.mobile.back();
    }
    touch::style(ui);
    match app.ui.mobile.route {
        Route::Home => home::show(app, ui),
        Route::NewDocument => new_document::show(app, ui),
        Route::Editor => editor(app, ui),
    }
}

/// The Editor: the canvas, the panel buttons and the open sheet.
fn editor(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    top_bar(app, ui, &t);
    sheet_bar(app, ui, &t);
    if let Some(sheet) = app.ui.mobile.sheet {
        sheet_panel(app, ui, &t, sheet);
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| crate::canvas::document_area(app, ui));
}

/// Back and the route's title.
fn top_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &crate::theme::Tokens) {
    egui::Panel::top("mobile-top-bar").exact_size(56.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(8, 0))).show(ui, |ui| {
        crate::direction::horizontal_centered(ui, |ui| {
            if app.ui.mobile.route != Route::Home && ui.add(egui::Button::new(tl!("Back")).min_size(egui::vec2(48.0, 48.0))).clicked() {
                app.ui.mobile.back();
            }
            let title = match app.ui.mobile.route {
                Route::Editor => app.session.active().map(|st| st.doc.name.clone()).unwrap_or_default(),
                route => crate::i18n::t(route.title()).to_string(),
            };
            ui.label(egui::RichText::new(title).strong());
        });
    });
}

/// The Editor's panel buttons, one per sheet.
fn sheet_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &crate::theme::Tokens) {
    egui::Panel::bottom("mobile-sheet-bar").exact_size(56.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(4, 0))).show(
        ui,
        |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                crate::direction::horizontal_centered(ui, |ui| {
                    for sheet in Sheet::ALL {
                        let open = app.ui.mobile.sheet == Some(sheet);
                        if ui.add(egui::Button::selectable(open, crate::i18n::t(sheet.title())).min_size(egui::vec2(48.0, 48.0))).clicked() {
                            // The Editor is the route here, so this cannot fail.
                            let _ = app.ui.mobile.toggle_sheet(sheet);
                        }
                    }
                });
            });
        },
    );
}

/// The open sheet: about two thirds of the height, over the bottom of the canvas.
fn sheet_panel(app: &mut PhotocraftApp, ui: &mut egui::Ui, t: &crate::theme::Tokens, sheet: Sheet) {
    let height = (ui.max_rect().height() * 0.62).max(160.0);
    egui::Panel::bottom("mobile-sheet").exact_size(height).frame(egui::Frame::NONE.fill(t.dock).inner_margin(egui::Margin::same(12))).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t(sheet.title())).strong());
            ui.with_layout(crate::direction::trailing(), |ui| {
                if ui.add(egui::Button::new(tl!("Close")).min_size(egui::vec2(48.0, 40.0))).clicked() {
                    app.ui.mobile.sheet = None;
                }
            });
        });
    });
}

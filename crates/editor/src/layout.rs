//! The layout of the editor panels as data (VIEW-018): widths, the side of the inspector, the tabs of the left panel in order, and the
//! size of a handle's hit area. A file of keys changes it without a rebuild; a missing key keeps its default; an unknown key or a value out
//! of range is refused with the key named.

use std::fmt;

/// A tab of the left panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// The edits near the camera and the search.
    Near,
    /// The edits by layer in the order they apply.
    Layers,
    /// The chain of edits from the procedural base to the tiles.
    Graph,
    /// The edits the engine cannot draw fully or that cost more than they should (VIEW-047), with fly-to and the fixes.
    Problems,
}

impl Tab {
    pub fn key(self) -> &'static str {
        match self {
            Tab::Near => "near",
            Tab::Layers => "layers",
            Tab::Graph => "graph",
            Tab::Problems => "problems",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Near => "Near",
            Tab::Layers => "Layers",
            Tab::Graph => "Graph",
            Tab::Problems => "Problems",
        }
    }

    fn of_key(s: &str) -> Option<Tab> {
        [Tab::Near, Tab::Layers, Tab::Graph, Tab::Problems].into_iter().find(|t| t.key() == s)
    }
}

/// The side of the window a panel sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The layout of the panels.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Width in points of the tool rail; 0 hides it.
    pub rail_width: f32,
    /// Width in points of the left panel (the tabs).
    pub left_width: f32,
    /// Width in points of the inspector.
    pub inspector_width: f32,
    pub inspector_side: Side,
    /// The tabs of the left panel, in order (at least one).
    pub tabs: Vec<Tab>,
    pub status_bar: bool,
    /// The radius in pixels around a handle in which a press takes it.
    pub handle_hit_px: f64,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            rail_width: 56.0,
            left_width: 300.0,
            inspector_width: 320.0,
            inspector_side: Side::Right,
            tabs: vec![Tab::Near, Tab::Layers, Tab::Graph, Tab::Problems],
            status_bar: true,
            handle_hit_px: 10.0,
        }
    }
}

/// Why a layout file was refused: the key and the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutError {
    pub key: String,
    pub why: String,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout `{}`: {}", self.key, self.why)
    }
}

impl std::error::Error for LayoutError {}

fn bad(key: &str, why: impl Into<String>) -> LayoutError {
    LayoutError { key: key.to_string(), why: why.into() }
}

const KEYS: [&str; 7] = ["rail_width", "left_width", "inspector_width", "inspector_side", "tabs", "status_bar", "handle_hit_px"];

impl Layout {
    /// Read a layout file; keys it does not name keep their defaults.
    pub fn from_toml(text: &str) -> Result<Layout, LayoutError> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| bad("(file)", e.to_string()))?;
        if let Some(k) = table.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(bad(k, format!("not a layout key (known: {})", KEYS.join(", "))));
        }
        let mut l = Layout::default();
        let number = |key: &str, lo: f64, hi: f64| -> Result<Option<f64>, LayoutError> {
            match table.get(key) {
                None => Ok(None),
                Some(v) => {
                    let n = v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).ok_or_else(|| bad(key, "a number"))?;
                    if n.is_finite() && (lo..=hi).contains(&n) {
                        Ok(Some(n))
                    } else {
                        Err(bad(key, format!("{n} is outside {lo}..{hi}")))
                    }
                }
            }
        };
        if let Some(n) = number("rail_width", 0.0, 200.0)? {
            l.rail_width = n as f32;
        }
        if let Some(n) = number("left_width", 120.0, 800.0)? {
            l.left_width = n as f32;
        }
        if let Some(n) = number("inspector_width", 120.0, 800.0)? {
            l.inspector_width = n as f32;
        }
        if let Some(n) = number("handle_hit_px", 3.0, 60.0)? {
            l.handle_hit_px = n;
        }
        if let Some(v) = table.get("inspector_side") {
            l.inspector_side = match v.as_str() {
                Some("left") => Side::Left,
                Some("right") => Side::Right,
                _ => return Err(bad("inspector_side", "`left` or `right`")),
            };
        }
        if let Some(v) = table.get("status_bar") {
            l.status_bar = v.as_bool().ok_or_else(|| bad("status_bar", "true or false"))?;
        }
        if let Some(v) = table.get("tabs") {
            let a = v.as_array().ok_or_else(|| bad("tabs", "a list of tab names"))?;
            let mut tabs = Vec::new();
            for t in a {
                let s = t.as_str().ok_or_else(|| bad("tabs", "a list of tab names"))?;
                let tab = Tab::of_key(s).ok_or_else(|| bad("tabs", format!("'{s}' is not a tab (near, layers, graph, problems)")))?;
                if tabs.contains(&tab) {
                    return Err(bad("tabs", format!("'{s}' is listed twice")));
                }
                tabs.push(tab);
            }
            if tabs.is_empty() {
                return Err(bad("tabs", "at least one tab"));
            }
            l.tabs = tabs;
        }
        Ok(l)
    }
}

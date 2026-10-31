use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Lucide icon files vendored unmodified under `resources/icons/` (ISC, see
/// `resources/icons/LICENSE`). Only icons used by `crate::icons::Icon` are
/// included (see `Icon::ALL` and its test).
macro_rules! lucide {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../../resources/icons/", $name, ".svg")))),*]
    };
}

static LUCIDE: &[(&str, &str)] = lucide!(
    "arrow-down-to-line",
    "check",
    "chevron-left",
    "chevron-right",
    "database",
    "external-link",
    "film",
    "folder",
    "heart",
    "image",
    "info",
    "maximize-2",
    "minimize-2",
    "minus",
    "panel-left",
    "pause",
    "pencil",
    "picture-in-picture-2",
    "play",
    "plus",
    "repeat",
    "rotate-ccw",
    "rotate-cw",
    "search",
    "settings",
    "sliders-vertical",
    "sparkles",
    "star",
    "skip-back",
    "skip-forward",
    "tag",
    "trash-2",
    "triangle-alert",
    "undo-2",
    "volume-2",
    "volume-x",
    "x",
    "zap",
);

/// Lucide is outline-only; these render solid (transport controls).
const SOLID: &[&str] = &["pause", "play", "skip-back", "skip-forward"];
/// Outline icons that also get a solid `<name>-filled` variant.
const FILLED_VARIANTS: &[&str] = &["heart", "star"];

fn solid(svg: &str) -> String {
    svg.replacen("fill=\"none\"", "fill=\"black\"", 1)
}

/// Asset path → SVG. `currentColor` becomes black: GPUI paints SVGs as an
/// alpha mask tinted by `text_color`, so only coverage matters.
static ICONS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for &(name, svg) in LUCIDE {
        let svg = svg.replace("currentColor", "black");
        if FILLED_VARIANTS.contains(&name) {
            m.insert(format!("icons/{name}-filled.svg"), solid(&svg));
        }
        let svg = if SOLID.contains(&name) {
            solid(&svg)
        } else {
            svg
        };
        m.insert(format!("icons/{name}.svg"), svg);
    }
    m
});

pub static PLEX_SANS_REGULAR: &[u8] =
    include_bytes!("../../../resources/fonts/IBMPlexSans-Regular.otf");
pub static PLEX_SANS_MEDIUM: &[u8] =
    include_bytes!("../../../resources/fonts/IBMPlexSans-Medium.otf");
pub static PLEX_SANS_SEMIBOLD: &[u8] =
    include_bytes!("../../../resources/fonts/IBMPlexSans-SemiBold.otf");
pub static PLEX_MONO_REGULAR: &[u8] =
    include_bytes!("../../../resources/fonts/IBMPlexMono-Regular.otf");

#[derive(Clone, Default)]
pub struct LuminaAssets;

impl AssetSource for LuminaAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(svg) = ICONS.get(path) {
            return Ok(Some(Cow::Borrowed(svg.as_bytes())));
        }

        match path {
            "fonts/IBMPlexSans-Regular.otf" => Ok(Some(Cow::Borrowed(PLEX_SANS_REGULAR))),
            "fonts/IBMPlexSans-Medium.otf" => Ok(Some(Cow::Borrowed(PLEX_SANS_MEDIUM))),
            "fonts/IBMPlexSans-SemiBold.otf" => Ok(Some(Cow::Borrowed(PLEX_SANS_SEMIBOLD))),
            "fonts/IBMPlexMono-Regular.otf" => Ok(Some(Cow::Borrowed(PLEX_MONO_REGULAR))),
            _ => Ok(None),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let prefix = if path.is_empty() { "" } else { path };
        let mut results = Vec::new();
        for key in ICONS.keys() {
            if key.starts_with(prefix) {
                results.push(key.clone().into());
            }
        }
        Ok(results)
    }
}

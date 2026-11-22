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
    "aperture",
    "arrow-down-to-line",
    "arrow-right",
    "arrow-up",
    "arrow-up-narrow-wide",
    "audio-lines",
    "calendar",
    "camera",
    "check",
    "check-check",
    "circle-alert",
    "circle-check",
    "circle-check-big",
    "clock",
    "combine",
    "copy",
    "corner-down-right",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "database",
    "download",
    "ellipsis",
    "eraser",
    "external-link",
    "file-image",
    "film",
    "folder",
    "folder-down",
    "folder-minus",
    "folder-open",
    "folder-plus",
    "folder-sync",
    "git-compare-arrows",
    "grid-3x3",
    "hard-drive",
    "hash",
    "heart",
    "image",
    "image-plus",
    "import",
    "images",
    "info",
    "layers",
    "layout-dashboard",
    "library",
    "link",
    "link-2",
    "list-ordered",
    "map",
    "map-pin",
    "maximize-2",
    "merge",
    "minimize-2",
    "minus",
    "panel-left",
    "panel-left-close",
    "panel-left-open",
    "panel-right",
    "pause",
    "pencil",
    "picture-in-picture-2",
    "play",
    "plus",
    "refresh-cw",
    "repeat",
    "rotate-ccw",
    "rotate-cw",
    "search",
    "search-x",
    "settings",
    "settings-2",
    "shield-check",
    "share-2",
    "sliders-horizontal",
    "sliders-vertical",
    "sparkles",
    "star",
    "skip-back",
    "skip-forward",
    "tag",
    "trash-2",
    "triangle-alert",
    "undo-2",
    "unplug",
    "video",
    "volume-2",
    "volume-x",
    "wand-sparkles",
    "wrench",
    "x",
    "zap",
    "zoom-in",
    "zoom-out",
);

/// The two-tone brand mark (`resources/brand/`). Drawn with `img()`, not
/// `render_icon`, which would tint it to one colour.
pub const BRAND_MARK: &str = "brand/lumina-mark.svg";
static BRAND_MARK_SVG: &str = include_str!("../../../resources/brand/lumina-mark.svg");

/// Lucide is outline-only; these render solid (transport controls).
const SOLID: &[&str] = &["pause", "play", "skip-back", "skip-forward"];
/// Outline icons that also get a solid `<name>-filled` variant.
const FILLED_VARIANTS: &[&str] = &["heart", "star"];

fn solid(svg: &str) -> String {
    svg.replacen("fill=\"none\"", "fill=\"black\"", 1)
}

/// Asset path → SVG. In Lucide icons `currentColor` becomes black: GPUI
/// paints them as an alpha mask tinted by `text_color`, so only coverage
/// matters. The brand mark keeps its colours.
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
    m.insert(BRAND_MARK.to_string(), BRAND_MARK_SVG.to_string());
    m
});

/// Bundled fonts (SIL OFL 1.1, see `THIRD_PARTY_NOTICES.md`), registered at
/// startup: Inter 400/500/600 (`theme::FONT_SANS`), Inter Tight 600 (the
/// wordmark and page titles, whose views name it) and JetBrains Mono 400
/// (`theme::FONT_MONO`).
pub static FONTS: [&[u8]; 5] = [
    include_bytes!("../../../resources/fonts/Inter-Regular.otf"),
    include_bytes!("../../../resources/fonts/Inter-Medium.otf"),
    include_bytes!("../../../resources/fonts/Inter-SemiBold.otf"),
    include_bytes!("../../../resources/fonts/InterTight-SemiBold.ttf"),
    include_bytes!("../../../resources/fonts/JetBrainsMono-Regular.ttf"),
];
/// Natural Earth 1:110m land (public domain), converted by
/// `scripts/build_world_basemap.py`; parsed by `app::map::parse_basemap`.
pub static LAND_110M: &[u8] = include_bytes!("../../../resources/map/land_110m.bin");

#[derive(Clone, Default)]
pub struct LuminaAssets;

impl AssetSource for LuminaAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS.get(path).map(|svg| Cow::Borrowed(svg.as_bytes())))
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

#[cfg(test)]
mod tests {
    use super::FONTS;

    fn utf16be(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    #[core::prelude::v1::test]
    fn bundled_fonts_carry_their_family_names() {
        assert_eq!(FONTS.len(), 5, "Inter x3, Inter Tight and JetBrains Mono");
        for (font, family) in
            FONTS
                .iter()
                .zip(["Inter", "Inter", "Inter", "Inter Tight", "JetBrains Mono"])
        {
            assert!(
                font.starts_with(b"OTTO") || font.starts_with(&[0, 1, 0, 0]),
                "{family}: every bundled font must be an OpenType/TrueType file"
            );
            let name = utf16be(family);
            assert!(
                font.windows(name.len()).any(|w| w == name),
                "{family}: the name table must carry the family theme.rs asks for"
            );
        }
    }
}

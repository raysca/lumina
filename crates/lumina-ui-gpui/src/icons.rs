use gpui::{svg, Pixels, Rgba, Styled, Svg};

/// Bundled Lucide icon set rendered as monochrome SVGs (tinted via `render_icon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Heart,
    HeartFilled,
    Star,
    StarFilled,
    Trash,
    X,
    Play,
    Pause,
    StepForward,
    StepBack,
    Pip,
    Maximize,
    Minimize,
    Volume,
    VolumeX,
    Image,
    ExternalLink,
    Undo2,
    Tag,
    Search,
    Info,
    PanelLeft,
    RotateCcw,
    RotateCw,
    Repeat,
    TriangleAlert,
    Sparkles,
    ArrowDownToLine,
    Check,
    ChevronLeft,
    ChevronRight,
    Zap,
    Folder,
    Film,
    Sliders,
    Plus,
    Minus,
    Pencil,
    Settings,
    Database,
    Images,
    Camera,
    MapPin,
    Copy,
    PanelLeftClose,
    PanelLeftOpen,
    Video,
    Clock,
    Aperture,
    SlidersHorizontal,
    ArrowUpNarrowWide,
    Import,
    Grid3x3,
    Map,
    Calendar,
    FileImage,
    ZoomIn,
    ZoomOut,
}

impl Icon {
    /// Every icon the UI can render. Kept in sync with the embedded assets so a
    /// missing SVG fails the asset-resolution test instead of silently rendering blank.
    #[allow(dead_code)] // exercised by `every_icon_resolves_in_asset_source`
    pub const ALL: &[Icon] = &[
        Icon::Heart,
        Icon::HeartFilled,
        Icon::Star,
        Icon::StarFilled,
        Icon::Trash,
        Icon::X,
        Icon::Play,
        Icon::Pause,
        Icon::StepForward,
        Icon::StepBack,
        Icon::Pip,
        Icon::Maximize,
        Icon::Minimize,
        Icon::Volume,
        Icon::VolumeX,
        Icon::Image,
        Icon::ExternalLink,
        Icon::Undo2,
        Icon::Tag,
        Icon::Search,
        Icon::Info,
        Icon::PanelLeft,
        Icon::RotateCcw,
        Icon::RotateCw,
        Icon::Repeat,
        Icon::TriangleAlert,
        Icon::Sparkles,
        Icon::ArrowDownToLine,
        Icon::Check,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::Zap,
        Icon::Folder,
        Icon::Film,
        Icon::Sliders,
        Icon::Plus,
        Icon::Minus,
        Icon::Pencil,
        Icon::Settings,
        Icon::Database,
        Icon::Images,
        Icon::Camera,
        Icon::MapPin,
        Icon::Copy,
        Icon::PanelLeftClose,
        Icon::PanelLeftOpen,
        Icon::Video,
        Icon::Clock,
        Icon::Aperture,
        Icon::SlidersHorizontal,
        Icon::ArrowUpNarrowWide,
        Icon::Import,
        Icon::Grid3x3,
        Icon::Map,
        Icon::Calendar,
        Icon::FileImage,
        Icon::ZoomIn,
        Icon::ZoomOut,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Self::Heart => "icons/heart.svg",
            Self::HeartFilled => "icons/heart-filled.svg",
            Self::Star => "icons/star.svg",
            Self::StarFilled => "icons/star-filled.svg",
            Self::Trash => "icons/trash-2.svg",
            Self::X => "icons/x.svg",
            Self::Play => "icons/play.svg",
            Self::Pause => "icons/pause.svg",
            Self::StepForward => "icons/skip-forward.svg",
            Self::StepBack => "icons/skip-back.svg",
            Self::Pip => "icons/picture-in-picture-2.svg",
            Self::Maximize => "icons/maximize-2.svg",
            Self::Minimize => "icons/minimize-2.svg",
            Self::Volume => "icons/volume-2.svg",
            Self::VolumeX => "icons/volume-x.svg",
            Self::Image => "icons/image.svg",
            Self::ExternalLink => "icons/external-link.svg",
            Self::Undo2 => "icons/undo-2.svg",
            Self::Tag => "icons/tag.svg",
            Self::Search => "icons/search.svg",
            Self::Info => "icons/info.svg",
            Self::PanelLeft => "icons/panel-left.svg",
            Self::RotateCcw => "icons/rotate-ccw.svg",
            Self::RotateCw => "icons/rotate-cw.svg",
            Self::Repeat => "icons/repeat.svg",
            Self::TriangleAlert => "icons/triangle-alert.svg",
            Self::Sparkles => "icons/sparkles.svg",
            Self::ArrowDownToLine => "icons/arrow-down-to-line.svg",
            Self::Check => "icons/check.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::Zap => "icons/zap.svg",
            Self::Folder => "icons/folder.svg",
            Self::Film => "icons/film.svg",
            Self::Sliders => "icons/sliders-vertical.svg",
            Self::Plus => "icons/plus.svg",
            Self::Minus => "icons/minus.svg",
            Self::Pencil => "icons/pencil.svg",
            Self::Settings => "icons/settings.svg",
            Self::Database => "icons/database.svg",
            Self::Images => "icons/images.svg",
            Self::Camera => "icons/camera.svg",
            Self::MapPin => "icons/map-pin.svg",
            Self::Copy => "icons/copy.svg",
            Self::PanelLeftClose => "icons/panel-left-close.svg",
            Self::PanelLeftOpen => "icons/panel-left-open.svg",
            Self::Video => "icons/video.svg",
            Self::Clock => "icons/clock.svg",
            Self::Aperture => "icons/aperture.svg",
            Self::SlidersHorizontal => "icons/sliders-horizontal.svg",
            Self::ArrowUpNarrowWide => "icons/arrow-up-narrow-wide.svg",
            Self::Import => "icons/import.svg",
            Self::Grid3x3 => "icons/grid-3x3.svg",
            Self::Map => "icons/map.svg",
            Self::Calendar => "icons/calendar.svg",
            Self::FileImage => "icons/file-image.svg",
            Self::ZoomIn => "icons/zoom-in.svg",
            Self::ZoomOut => "icons/zoom-out.svg",
        }
    }
}

/// Create an icon element with custom pixel size and color.
pub fn render_icon(i: Icon, size: Pixels, color: Rgba) -> Svg {
    svg().path(i.path()).size(size).text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource as _;

    #[test]
    fn every_icon_resolves_in_asset_source() {
        let assets = crate::assets::LuminaAssets;
        for icon in Icon::ALL {
            let loaded = assets.load(icon.path()).unwrap_or_else(|err| {
                panic!("asset load failed for {:?} ({}): {err}", icon, icon.path())
            });
            let Some(bytes) = loaded else {
                panic!("no asset registered for {:?} at {}", icon, icon.path());
            };
            let svg = std::str::from_utf8(&bytes).expect("icons are UTF-8 SVG");
            assert!(
                svg.contains("<svg") && !svg.contains("currentColor"),
                "{:?} must be an SVG with black paint for GPUI alpha masking",
                icon
            );
        }
        use Icon::*;
        for icon in [HeartFilled, StarFilled, Play, Pause, StepBack, StepForward] {
            let bytes = assets.load(icon.path()).unwrap().unwrap();
            let svg = std::str::from_utf8(&bytes).unwrap();
            let root = &svg[svg.find("<svg").unwrap()..];
            assert!(
                root[..root.find('>').unwrap()].contains("fill=\"black\""),
                "{icon:?} must render solid"
            );
        }
    }
}

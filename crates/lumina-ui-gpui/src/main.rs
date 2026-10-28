#![recursion_limit = "4096"]

mod app;
mod assets;
mod bridge;
mod config;
mod icons;
mod player;
mod theme;
mod views;

pub(crate) use app::*;

use gpui::*;

#[cfg(test)]
mod tests;

fn main() {
    Application::new()
        .with_assets(assets::LuminaAssets)
        .run(|cx: &mut App| {
            // Register bundled IBM Plex fonts
            if let Err(e) = cx.text_system().add_fonts(vec![
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_REGULAR),
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_MEDIUM),
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_SEMIBOLD),
                std::borrow::Cow::Borrowed(assets::PLEX_MONO_REGULAR),
            ]) {
                eprintln!("Warning: Failed to load bundled IBM Plex fonts: {:?}", e);
            }

            let bounds = Bounds::centered(None, size(px(1280.0), px(840.0)), cx);
            if let Err(e) = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Lumina".into()),
                        appears_transparent: true,
                        // Centred in the 52px toolbar.
                        traffic_light_position: Some(point(px(18.0), px(19.0))),
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(LuminaApp::new),
            ) {
                eprintln!("Failed to open window: {:?}", e);
            }
            cx.activate(true);
        });
}

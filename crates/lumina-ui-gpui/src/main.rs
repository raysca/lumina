#![recursion_limit = "4096"]

mod actions;
mod app;
mod assets;
mod bridge;
mod config;
mod i18n;
mod icons;
mod instance_lock;
mod logging;
mod menus;
mod motion;
mod player;
mod theme;
mod views;

pub(crate) use app::*;

use gpui::*;

#[cfg(test)]
mod tests;

fn main() {
    logging::init();

    // Only one Lumina process may touch a catalog at a time.
    let _instance_lock = match instance_lock::acquire() {
        Ok(lock) => {
            log::info!("instance lock acquired at {}", lock.path().display());
            lock
        }
        Err(err) => {
            // A blocking modal here would hang automated launches; log and exit.
            log::error!("{err}");
            eprintln!("Lumina: {err}");
            return;
        }
    };

    Application::new()
        .with_assets(assets::LuminaAssets)
        .run(|cx: &mut App| {
            cx.set_menus(crate::menus::build_menus());

            // Register bundled IBM Plex fonts
            if let Err(e) = cx.text_system().add_fonts(vec![
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_REGULAR),
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_MEDIUM),
                std::borrow::Cow::Borrowed(assets::PLEX_SANS_SEMIBOLD),
                std::borrow::Cow::Borrowed(assets::PLEX_MONO_REGULAR),
            ]) {
                log::warn!("failed to load bundled IBM Plex fonts: {e:?}");
            }

            let restored = crate::config::AppConfig::load().restored_window_bounds();
            let bounds = match restored {
                Some((x, y, width, height)) => {
                    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
                }
                None => Bounds::centered(None, size(px(1280.0), px(840.0)), cx),
            };
            if let Err(e) = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(960.0), px(600.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Lumina".into()),
                        appears_transparent: true,
                        // Centred in the 52px toolbar.
                        traffic_light_position: Some(point(px(18.0), px(19.0))),
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| LuminaApp::new_in_window(window, cx)),
            ) {
                log::error!("failed to open main window: {e:?}");
            }
            cx.activate(true);
        });
}

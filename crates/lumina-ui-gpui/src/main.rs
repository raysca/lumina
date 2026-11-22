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
#[cfg(target_os = "macos")]
mod share;
mod storage;
mod theme;
mod views;

pub(crate) use app::*;

use gpui::*;

/// The AppStream/desktop id. Also the window's Wayland app_id and X11
/// WM_CLASS, so desktops and the Flatpak portal group the window under the
/// same name as the installed entry.
const APP_ID: &str = "io.github.raysca.Lumina";

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
            log::error!("{err:#}");
            eprintln!("Lumina: {err:#}");
            return;
        }
    };

    gpui::application()
        .with_assets(assets::LuminaAssets)
        .run(|cx: &mut App| {
            // One parse for the menus, the window bounds and the app itself.
            // The corrupt-file message is cached with it and surfaced by
            // LuminaApp::new, so an earlier read cannot consume the file first.
            let (startup, _settings_error) = crate::config::AppConfig::load_startup_once();
            cx.set_menus(crate::menus::build_menus(!startup.sidebar_visible));
            // The text fields' input key bindings and globals.
            gpui::init(cx);

            if let Err(e) = cx
                .text_system()
                .add_fonts(assets::FONTS.map(std::borrow::Cow::Borrowed).to_vec())
            {
                log::warn!("failed to load bundled fonts: {e:?}");
            }

            let restored = startup.restored_window_bounds();
            let bounds = match restored {
                Some((x, y, width, height)) => {
                    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
                }
                None => Bounds::centered(None, size(px(1280.0), px(840.0)), cx),
            };
            #[cfg(target_os = "macos")]
            let titlebar = Some(TitlebarOptions {
                title: Some("Lumina".into()),
                appears_transparent: true,
                // Centred on the title bar band's line (theme::TITLEBAR_HEIGHT / 2:
                // the buttons are 14pt tall), ending before theme::TRAFFIC_LIGHTS_INSET.
                traffic_light_position: Some(point(px(18.0), px(19.0))),
            });
            #[cfg(not(target_os = "macos"))]
            let titlebar = Some(TitlebarOptions {
                title: Some("Lumina".into()),
                appears_transparent: false,
                traffic_light_position: None,
            });
            match cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(960.0), px(600.0))),
                    titlebar,
                    app_id: Some(APP_ID.into()),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| LuminaApp::new_in_window(window, cx)),
            ) {
                Ok(_) => log::info!("main window opened"),
                Err(e) => log::error!("failed to open main window: {e:?}"),
            }
            cx.activate(true);
        });
}

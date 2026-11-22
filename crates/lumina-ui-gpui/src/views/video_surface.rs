//! The live video frame, painted from the player's GPU surface.

use gpui::*;

use crate::player::VideoSurface;

/// `surface` filling its (relative) parent, contain-fitted like the frame
/// image under it, which keeps the clicks, the size and the shadow. On
/// platforms without surfaces this is nothing and the image shows.
///
/// Square corners: GPUI's surface element cannot round them. Corner masks,
/// if wanted, go here.
pub(crate) fn video_surface_layer(surface: Option<&VideoSurface>) -> Option<AnyElement> {
    #[cfg(target_os = "macos")]
    {
        surface.map(|surface| {
            div()
                .debug_selector(|| "video-surface".to_string())
                .absolute()
                .inset_0()
                .child(
                    gpui::surface(surface.clone())
                        .object_fit(ObjectFit::Contain)
                        .size_full(),
                )
                .into_any_element()
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        surface.map(|surface| match *surface {})
    }
}

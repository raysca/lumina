use anyhow::{anyhow, Result};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use lumina_core::{DecodedFrame, PixelFormat};
use smallvec::smallvec;
use std::sync::Arc;

pub fn frame_to_gpui_image(frame: DecodedFrame) -> Result<Arc<RenderImage>> {
    let bgra_data = match frame.format {
        PixelFormat::Rgba8 => {
            let mut data = frame.data;
            // GPUI Metal texture atlas uses BGRA8Unorm pixel format.
            // Convert RGBA8 to BGRA8 by swapping Red (0) and Blue (2) channels.
            for chunk in data.as_chunks_mut::<4>().0 {
                chunk.swap(0, 2);
            }
            data
        }
        PixelFormat::Bgra8 => frame.data,
    };

    let buffer = RgbaImage::from_raw(frame.width, frame.height, bgra_data)
        .ok_or_else(|| anyhow!("Failed to construct image buffer from frame data"))?;

    let frame_element = Frame::new(buffer);
    Ok(Arc::new(RenderImage::new(smallvec![frame_element])))
}

pub fn video_frame_to_gpui_image(frame: crate::player::VideoFrame) -> Result<Arc<RenderImage>> {
    let buffer = RgbaImage::from_raw(frame.width, frame.height, frame.data)
        .ok_or_else(|| anyhow!("Failed to construct image buffer from video frame"))?;

    let frame_element = Frame::new(buffer);
    Ok(Arc::new(RenderImage::new(smallvec![frame_element])))
}

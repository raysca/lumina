use anyhow::{anyhow, Result};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use lumina_core::{DecodedFrame, PixelFormat};
use smallvec::smallvec;
use std::sync::Arc;

pub fn frame_to_gpui_image(frame: DecodedFrame) -> Result<Arc<RenderImage>> {
    let rgba_buffer = match frame.format {
        PixelFormat::Rgba8 => {
            RgbaImage::from_raw(frame.width, frame.height, frame.data)
                .ok_or_else(|| anyhow!("Failed to construct RgbaImage from frame data"))?
        }
        PixelFormat::Bgra8 => {
            let mut data = frame.data;
            for chunk in data.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            RgbaImage::from_raw(frame.width, frame.height, data)
                .ok_or_else(|| anyhow!("Failed to construct RgbaImage from frame data"))?
        }
    };

    let frame_element = Frame::new(rgba_buffer);
    Ok(Arc::new(RenderImage::new(smallvec![frame_element])))
}

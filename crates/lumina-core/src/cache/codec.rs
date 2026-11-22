//! The on-disk encoding of one cached decode: a 16-byte header, then the
//! pixels as JPEG (quality 85) when the frame is opaque, or PNG when any
//! pixel is transparent (JPEG has no alpha). Raw pixels cost 4 bytes each,
//! so a 1.5 GB cache held about 1,900 grid thumbnails; compressed it holds
//! tens of thousands.

use anyhow::{anyhow, bail, Result};
use image::ImageEncoder;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::{DecodedFrame, MediaId, PixelFormat};

/// JPEG quality for opaque entries: invisible at grid sizes, about a
/// tenth of the raw size for a photo.
const JPEG_QUALITY: u8 = 85;
const CODEC_JPEG: u32 = 1;
const CODEC_PNG: u32 = 2;
const HEADER_LEN: usize = 16;

/// `[u32 width, u32 height, u32 pixel format, u32 codec]`, little endian,
/// then the encoded image.
pub(super) fn encode(frame: &DecodedFrame) -> Result<Vec<u8>> {
    let (width, height) = (frame.width, frame.height);
    if frame.data.len() != width as usize * height as usize * 4 {
        bail!("frame data does not match {width}x{height}");
    }
    // Channel order is restored on decode; the encoders take RGB(A).
    let (r, b) = match frame.format {
        PixelFormat::Rgba8 => (0, 2),
        PixelFormat::Bgra8 => (2, 0),
    };
    let pixels = frame.data.as_chunks::<4>().0;
    let opaque = pixels.iter().all(|px| px[3] == 255);
    let mut out = Vec::with_capacity(HEADER_LEN + frame.data.len() / 8);
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    let format_tag: u32 = match frame.format {
        PixelFormat::Rgba8 => 0,
        PixelFormat::Bgra8 => 1,
    };
    out.extend_from_slice(&format_tag.to_le_bytes());
    if opaque {
        out.extend_from_slice(&CODEC_JPEG.to_le_bytes());
        let rgb: Vec<u8> = pixels.iter().flat_map(|px| [px[r], px[1], px[b]]).collect();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode(
            &rgb,
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )?;
    } else {
        out.extend_from_slice(&CODEC_PNG.to_le_bytes());
        let rgba: Vec<u8> = pixels
            .iter()
            .flat_map(|px| [px[r], px[1], px[b], px[3]])
            .collect();
        image::codecs::png::PngEncoder::new(&mut out).write_image(
            &rgba,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
    }
    Ok(out)
}

/// The frame `bytes` holds, in the pixel format it was written in; `None`
/// for a short, foreign or corrupt entry (read as a miss).
pub(super) fn decode(id: MediaId, bytes: &[u8]) -> Option<DecodedFrame> {
    let field = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let (width, height) = (field(0)?, field(4)?);
    let format = match field(8)? {
        0 => PixelFormat::Rgba8,
        1 => PixelFormat::Bgra8,
        _ => return None,
    };
    let codec = field(12)?;
    let body = bytes.get(HEADER_LEN..)?;
    let data = match codec {
        CODEC_JPEG => decode_jpeg(body, format),
        CODEC_PNG => decode_png(body, format),
        _ => Err(anyhow!("unknown codec")),
    }
    .map_err(|err| log::warn!("unreadable thumbnail cache entry: {err:#}"))
    .ok()?;
    (data.len() == width as usize * height as usize * 4).then_some(DecodedFrame {
        id,
        width,
        height,
        format,
        data,
    })
}

fn decode_jpeg(body: &[u8], format: PixelFormat) -> Result<Vec<u8>> {
    let colorspace = match format {
        PixelFormat::Rgba8 => ColorSpace::RGBA,
        PixelFormat::Bgra8 => ColorSpace::BGRA,
    };
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    Ok(JpegDecoder::new_with_options(body, options).decode()?)
}

fn decode_png(body: &[u8], format: PixelFormat) -> Result<Vec<u8>> {
    let mut data = image::load_from_memory_with_format(body, image::ImageFormat::Png)?
        .into_rgba8()
        .into_raw();
    if format == PixelFormat::Bgra8 {
        for px in data.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(format: PixelFormat, alpha: u8) -> DecodedFrame {
        let data = (0..24u32 * 16)
            .flat_map(|i| [(i * 7 % 256) as u8, (i % 24 * 10) as u8, 200, alpha])
            .collect();
        DecodedFrame {
            id: MediaId(3),
            width: 24,
            height: 16,
            format,
            data,
        }
    }

    /// An opaque frame is stored as JPEG and comes back in its own channel
    /// order, close to the original.
    #[test]
    fn opaque_frames_round_trip_through_jpeg_in_their_channel_order() {
        for format in [PixelFormat::Bgra8, PixelFormat::Rgba8] {
            let original = frame(format, 255);
            let bytes = encode(&original).unwrap();
            assert_eq!(
                &bytes[12..16],
                &CODEC_JPEG.to_le_bytes(),
                "{format:?}: JPEG"
            );
            let back = decode(MediaId(9), &bytes).expect("decodes");
            assert_eq!(
                (back.id, back.width, back.height, back.format),
                (MediaId(9), 24, 16, format)
            );
            let mean: f64 = back
                .data
                .iter()
                .zip(&original.data)
                .map(|(&a, &b)| f64::from(a.abs_diff(b)))
                .sum::<f64>()
                / original.data.len() as f64;
            assert!(
                mean < 8.0,
                "{format:?}: channels must not be swapped or lost, mean error {mean}"
            );
        }
    }

    /// JPEG has no alpha: a frame with transparency is stored losslessly.
    #[test]
    fn transparent_frames_round_trip_exactly_through_png() {
        let original = frame(PixelFormat::Bgra8, 100);
        let bytes = encode(&original).unwrap();
        assert_eq!(&bytes[12..16], &CODEC_PNG.to_le_bytes());
        let back = decode(MediaId(3), &bytes).expect("decodes");
        assert_eq!(back.data, original.data, "PNG keeps every pixel and alpha");
    }

    #[test]
    fn foreign_bytes_are_not_a_frame() {
        assert!(decode(MediaId(1), b"").is_none());
        assert!(
            decode(MediaId(1), &[0u8; 16]).is_none(),
            "codec 0 is unknown"
        );
        let mut bytes = encode(&frame(PixelFormat::Bgra8, 255)).unwrap();
        bytes[0] = 99;
        assert!(
            decode(MediaId(1), &bytes).is_none(),
            "the size must match the header"
        );
    }
}

//! Pixel ops for non-destructive quick edits. They run on a decoded frame
//! after the disk cache, so the original file and cached decodes are never
//! changed. `decoder.rs` stays decode-only.

use crate::catalog::{CropRect, Edits};
use crate::{DecodedFrame, DecoderPipeline, MediaType, PixelFormat};

/// Largest decode an edit may ask for (a 4096² RGBA frame is ~64 MB).
const MAX_EDIT_DECODE: u32 = 4096;

/// Applies `edits` in a fixed order: crop (unrotated coordinates), rotation,
/// then one tone LUT (exposure composed with auto-enhance). With no crop,
/// exposure or auto-enhance this is exactly [`DecoderPipeline::rotate_frame`].
pub fn apply_edits(frame: DecodedFrame, edits: &Edits) -> DecodedFrame {
    let frame = match edits.crop {
        Some(crop) => crop_frame(frame, crop),
        None => frame,
    };
    let mut frame = DecoderPipeline::rotate_frame(frame, edits.rotation);
    if edits.exposure_ev != 0.0 || edits.auto_enhance {
        apply_tone(&mut frame, edits.exposure_ev, edits.auto_enhance);
    }
    frame
}

/// Largest edit decode that is written to the disk cache: a crop above it
/// is decoded per request, so one cropped viewer item (2048 px, decoded at up
/// to 4096) cannot fill the cache budget with ~64 MB files.
const MAX_CACHED_EDIT_DECODE: u32 = 2048;

/// The edits a decode of `media_type` honours: videos keep only rotation.
pub fn effective_edits(media_type: MediaType, edits: Edits) -> Edits {
    match media_type {
        MediaType::Video => Edits {
            rotation: edits.rotation,
            ..Edits::default()
        },
        MediaType::Image | MediaType::Raw => edits,
    }
}

/// The decode size (long side) that leaves `crop` at least `target` pixels
/// on its long side. `original` is the catalog's `(width, height)`, `(0, 0)`
/// when unknown: the displayed size, after EXIF orientation, like the crop.
/// Rows not yet fixed by the one-time size backfill may still hold the
/// stored (unrotated) size, so the result covers both readings.
/// Never below `target`; capped at 4096 and at the original's long side.
pub fn edit_decode_size(target: u32, crop: Option<CropRect>, original: (u32, u32)) -> u32 {
    let Some(crop) = crop else {
        return target;
    };
    let (w, h) = (crop.w.clamp(0.02, 1.0), crop.h.clamp(0.02, 1.0));
    let (width, height) = (original.0 as f32, original.1 as f32);
    let long = width.max(height);
    // Crop long side as a fraction of the frame's long side.
    let fraction = if long > 0.0 {
        let upright = (w * width).max(h * height);
        let turned = (w * height).max(h * width);
        upright.min(turned) / long
    } else {
        w.max(h)
    };
    let mut cap = MAX_EDIT_DECODE;
    if long > 0.0 {
        // sips upscales; never decode beyond the original.
        cap = cap.min(long as u32);
    }
    ((target as f32 / fraction).ceil() as u32)
        .min(cap)
        .max(target)
}

/// Whether an edit decode at `decode_size` for a `target` request may be
/// written to the disk cache. The unedited size always is.
pub fn edit_decode_is_cacheable(target: u32, decode_size: u32) -> bool {
    decode_size == target
        || (decode_size <= target.saturating_mul(2) && decode_size <= MAX_CACHED_EDIT_DECODE)
}

/// Pixel rect of a normalized crop: rounded, clamped, at least 1×1.
fn crop_frame(frame: DecodedFrame, crop: CropRect) -> DecodedFrame {
    let (width, height) = (frame.width, frame.height);
    if width == 0 || height == 0 || frame.data.len() != width as usize * height as usize * 4 {
        return frame;
    }
    let edge = |v: f32, len: u32| ((v * len as f32).round().max(0.0) as u32).min(len);
    let x0 = edge(crop.x, width).min(width - 1);
    let y0 = edge(crop.y, height).min(height - 1);
    let x1 = edge(crop.x + crop.w, width).max(x0 + 1);
    let y1 = edge(crop.y + crop.h, height).max(y0 + 1);
    let row = |y: u32| {
        let start = (y as usize * width as usize + x0 as usize) * 4;
        &frame.data[start..start + (x1 - x0) as usize * 4]
    };
    let data = (y0..y1).flat_map(row).copied().collect();
    DecodedFrame {
        width: x1 - x0,
        height: y1 - y0,
        data,
        ..frame
    }
}

/// One LUT over R, G and B (alpha untouched): exposure in linear light with
/// a 2.2 gamma approximation of sRGB, then the auto-enhance stretch.
fn apply_tone(frame: &mut DecodedFrame, exposure_ev: f32, auto_enhance: bool) {
    let gain = 2f32.powf(exposure_ev);
    let exposure: [u8; 256] = std::array::from_fn(|v| {
        let linear = (v as f32 / 255.0).powf(2.2) * gain;
        (linear.min(1.0).powf(1.0 / 2.2) * 255.0).round() as u8
    });
    let lut = match auto_enhance.then(|| luminance_range(frame)).flatten() {
        Some((lo, hi)) => {
            let (lo, hi) = (exposure[lo as usize] as f32, exposure[hi as usize] as f32);
            if hi - lo < 16.0 {
                exposure
            } else {
                exposure.map(|v| {
                    ((v as f32 - lo) * 255.0 / (hi - lo))
                        .round()
                        .clamp(0.0, 255.0) as u8
                })
            }
        }
        None => exposure,
    };
    for pixel in frame.data.as_chunks_mut::<4>().0 {
        for channel in &mut pixel[..3] {
            *channel = lut[*channel as usize];
        }
    }
}

/// The 0.5 % and 99.5 % luminance percentiles from a 256-bin histogram.
fn luminance_range(frame: &DecodedFrame) -> Option<(u8, u8)> {
    let (r, b) = match frame.format {
        PixelFormat::Rgba8 => (0, 2),
        PixelFormat::Bgra8 => (2, 0),
    };
    let mut histogram = [0u64; 256];
    for pixel in frame.data.as_chunks::<4>().0 {
        // Rec. 709 weights in 8.8 fixed point.
        let luma = (54 * pixel[r] as u32 + 183 * pixel[1] as u32 + 19 * pixel[b] as u32) >> 8;
        histogram[luma as usize] += 1;
    }
    let total: u64 = histogram.iter().sum();
    if total == 0 {
        return None;
    }
    let percentile = |fraction: f64| {
        let threshold = (total as f64 * fraction).ceil().max(1.0) as u64;
        let mut seen = 0;
        histogram
            .iter()
            .position(|&count| {
                seen += count;
                seen >= threshold
            })
            .unwrap_or(255) as u8
    };
    Some((percentile(0.005), percentile(0.995)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaId;

    /// A `width` × `height` RGBA frame whose pixel (x, y) is `[x, y, 0, 255]`
    /// scaled by 10, so every pixel names its own position.
    fn grid(width: u32, height: u32) -> DecodedFrame {
        let data = (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| [x as u8 * 10, y as u8 * 10, 0, 255]))
            .collect();
        DecodedFrame {
            id: MediaId(1),
            width,
            height,
            format: PixelFormat::Rgba8,
            data,
        }
    }

    fn solid(values: &[u8]) -> DecodedFrame {
        DecodedFrame {
            id: MediaId(1),
            width: values.len() as u32,
            height: 1,
            format: PixelFormat::Bgra8,
            data: values.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        }
    }

    fn positions(frame: &DecodedFrame) -> Vec<(u8, u8)> {
        frame
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| (p[0] / 10, p[1] / 10))
            .collect()
    }

    fn crop(x: f32, y: f32, w: f32, h: f32) -> Edits {
        Edits {
            crop: Some(CropRect { x, y, w, h }),
            ..Default::default()
        }
    }

    #[test]
    fn crop_selects_expected_pixels() {
        let out = apply_edits(grid(4, 4), &crop(0.25, 0.5, 0.5, 0.5));
        assert_eq!((out.width, out.height), (2, 2), "half of a 4x4 frame");
        assert_eq!(
            positions(&out),
            [(1, 2), (2, 2), (1, 3), (2, 3)],
            "the crop keeps columns 1-2 of rows 2-3, in order"
        );
    }

    #[test]
    fn rotation_after_crop_keeps_region() {
        let edits = Edits {
            rotation: 1,
            ..crop(0.0, 0.0, 0.5, 0.5)
        };
        let out = apply_edits(grid(4, 4), &edits);
        assert_eq!((out.width, out.height), (2, 2));
        // Top-left quadrant [(0,0) (1,0) / (0,1) (1,1)] turned clockwise.
        assert_eq!(
            positions(&out),
            [(0, 1), (0, 0), (1, 1), (1, 0)],
            "a rotated crop shows the same quadrant turned, not another quadrant"
        );
    }

    #[test]
    fn exposure_plus_one_ev_brightens_midtones_and_clips_white() {
        let edits = Edits {
            exposure_ev: 1.0,
            ..Default::default()
        };
        let out = apply_edits(solid(&[0, 128, 250, 255]), &edits);
        let px = |i: usize| &out.data[i * 4..i * 4 + 4];
        assert_eq!(px(0), [0, 0, 0, 255], "black stays black");
        // 2^(1/2.2) ≈ 1.37: mid grey 128 lands near 175.
        assert!(
            (170..=180).contains(&px(1)[0]),
            "+1 EV brightens a midtone by about 1.37x in sRGB: {:?}",
            px(1)
        );
        assert_eq!(px(2), [255, 255, 255, 255], "near-white clips to white");
        assert_eq!(px(3)[3], 255, "alpha is untouched");
    }

    #[test]
    fn auto_enhance_stretches_low_contrast() {
        let values: Vec<u8> = (0..200).map(|i| 100 + (i % 51) as u8).collect();
        let edits = Edits {
            auto_enhance: true,
            ..Default::default()
        };
        let out = apply_edits(solid(&values), &edits);
        let lumas: Vec<u8> = out.data.as_chunks::<4>().0.iter().map(|p| p[1]).collect();
        let (min, max) = (lumas.iter().min(), lumas.iter().max());
        assert_eq!(
            (min, max),
            (Some(&0), Some(&255)),
            "a 100-150 range stretches to the full 0-255"
        );
    }

    #[test]
    fn auto_enhance_leaves_flat_image_unchanged() {
        let values: Vec<u8> = (0..100).map(|i| 120 + (i % 10) as u8).collect();
        let source = solid(&values);
        let before = source.data.clone();
        let edits = Edits {
            auto_enhance: true,
            ..Default::default()
        };
        assert_eq!(
            apply_edits(source, &edits).data,
            before,
            "a range under 16 levels is not stretched (no noise amplification)"
        );
    }

    #[test]
    fn edit_decode_size_caps() {
        let crop = |w, h| {
            Some(CropRect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            })
        };
        let unknown = (0, 0);
        assert_eq!(
            edit_decode_size(256, None, (4000, 3000)),
            256,
            "no crop: the target"
        );
        assert_eq!(
            edit_decode_size(256, crop(0.5, 0.25), unknown),
            512,
            "long side"
        );
        assert_eq!(
            edit_decode_size(100, crop(0.3, 0.3), unknown),
            334,
            "rounds up"
        );
        assert_eq!(
            edit_decode_size(2048, crop(0.02, 0.02), (9000, 6000)),
            4096,
            "a tiny crop is capped at 4096"
        );
        assert_eq!(
            edit_decode_size(1024, crop(0.1, 0.1), (1600, 1200)),
            1600,
            "never decoded beyond the original's long side"
        );
        assert_eq!(
            edit_decode_size(2048, crop(0.5, 0.5), (1000, 800)),
            2048,
            "never below the target (the unedited decode size)"
        );
    }

    #[test]
    fn edit_decode_size_uses_aspect() {
        // A tall crop on a 2:1 landscape: the crop's long side is its height,
        // half the frame's long side, so the decode doubles.
        let tall = Some(CropRect {
            x: 0.0,
            y: 0.0,
            w: 0.3,
            h: 1.0,
        });
        let size = edit_decode_size(1000, tall, (4000, 2000));
        assert_eq!(size, 2000, "aspect-aware decode size");
        let crop_height = size as f32 / 2.0;
        assert!(
            crop_height >= 1000.0,
            "the crop's long side reaches the target: {crop_height}"
        );
        // Stored 2000x4000 (portrait or a turned landscape) gives the same.
        assert_eq!(edit_decode_size(1000, tall, (2000, 4000)), 2000);
    }

    #[test]
    fn oversize_edit_decodes_are_not_cacheable() {
        assert!(
            edit_decode_is_cacheable(2048, 2048),
            "the unedited size caches"
        );
        assert!(edit_decode_is_cacheable(320, 640), "a 2x grid crop caches");
        assert!(!edit_decode_is_cacheable(320, 641), "over 2x does not");
        assert!(
            !edit_decode_is_cacheable(2048, 4096),
            "a cropped viewer decode (4096 for 2048) is never cached"
        );
        assert!(!edit_decode_is_cacheable(1500, 2100), "over 2048 does not");
    }

    #[test]
    fn videos_keep_only_rotation() {
        let edits = Edits {
            rotation: 3,
            exposure_ev: 1.0,
            auto_enhance: true,
            ..crop(0.0, 0.0, 0.5, 0.5)
        };
        assert_eq!(
            effective_edits(MediaType::Video, edits),
            Edits {
                rotation: 3,
                ..Default::default()
            },
            "videos ignore crop, exposure and auto-enhance"
        );
        assert_eq!(effective_edits(MediaType::Image, edits), edits);
        assert_eq!(effective_edits(MediaType::Raw, edits), edits);
    }

    #[test]
    fn rotation_only_matches_rotate_frame() {
        for quarters in 0..4 {
            let edits = Edits {
                rotation: quarters,
                ..Default::default()
            };
            assert_eq!(
                apply_edits(grid(4, 3), &edits).data,
                DecoderPipeline::rotate_frame(grid(4, 3), quarters).data,
                "rotation alone is exactly rotate_frame ({quarters} turns)"
            );
        }
    }
}

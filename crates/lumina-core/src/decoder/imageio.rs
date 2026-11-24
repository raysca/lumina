//! In-process ImageIO decoding (`image_decode_macos.m`). It replaced a
//! `sips` process per decode and a PNG round trip through a temp file.

use anyhow::{anyhow, Result};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{DecodedFrame, MediaId, PixelFormat};

extern "C" {
    fn lumina_image_decode(
        path: *const c_char,
        max_size: c_int,
        out_width: *mut c_int,
        out_height: *mut c_int,
        alloc: extern "C" fn(ctx: *mut c_void, len: usize) -> *mut u8,
        ctx: *mut c_void,
    ) -> c_int;
    fn lumina_image_pixel_size(
        path: *const c_char,
        out_width: *mut c_int,
        out_height: *mut c_int,
    ) -> c_int;
}

/// Gives the bridge the frame's buffer: `ctx` is the decode's `Vec<u8>`,
/// replaced by `len` zeroed bytes (the bridge draws over transparent black).
extern "C" fn alloc_frame(ctx: *mut c_void, len: usize) -> *mut u8 {
    // SAFETY: `decode` passes a pointer to its own live `Vec<u8>`, used by
    // nothing else during the call.
    let buffer = unsafe { &mut *ctx.cast::<Vec<u8>>() };
    // An impossible size is refused rather than aborting the process.
    if image::Limits::default().reserve(len as u64).is_err() {
        return std::ptr::null_mut();
    }
    *buffer = vec![0u8; len];
    buffer.as_mut_ptr()
}

/// `path` fitted inside `target_size` (never enlarged), turned by its EXIF
/// orientation and colour matched to sRGB, as straight-alpha BGRA8. The
/// bridge draws straight into the frame's buffer: no copy.
pub(crate) fn decode(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let max_size = c_int::try_from(target_size)?;
    let (mut width, mut height) = (0, 0);
    let mut data: Vec<u8> = Vec::new();
    // SAFETY: the path is a valid C string; the out-pointers and `data`
    // live for the call, and `alloc_frame` only touches `data`.
    let ok = unsafe {
        lumina_image_decode(
            c_path.as_ptr(),
            max_size,
            &mut width,
            &mut height,
            alloc_frame,
            (&mut data as *mut Vec<u8>).cast::<c_void>(),
        )
    };
    let (width, height) = (width as u32, height as u32);
    if ok == 0 || data.len() != width as usize * height as usize * 4 || data.is_empty() {
        return Err(anyhow!("ImageIO could not decode {}", path.display()));
    }
    Ok(DecodedFrame {
        id,
        width,
        height,
        format: PixelFormat::Bgra8,
        data,
    })
}

/// The stored (unrotated) pixel size of `path`; `None` when ImageIO cannot
/// read it.
pub(crate) fn pixel_size(path: &Path) -> Option<(u32, u32)> {
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let (mut width, mut height) = (0, 0);
    // SAFETY: the path is a valid C string; the out-pointers live for the call.
    let ok = unsafe { lumina_image_pixel_size(c_path.as_ptr(), &mut width, &mut height) };
    (ok != 0).then_some((width as u32, height as u32))
}

//! In-process ImageIO decoding (`image_decode_macos.m`). It replaced a
//! `sips` process per decode and a PNG round trip through a temp file.

use anyhow::{anyhow, Result};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{DecodedFrame, MediaId, PixelFormat};

extern "C" {
    fn lumina_image_decode(
        path: *const c_char,
        max_size: c_int,
        out_width: *mut c_int,
        out_height: *mut c_int,
        out_data: *mut *mut u8,
    ) -> c_int;
    fn lumina_image_free(data: *mut u8);
    fn lumina_image_pixel_size(
        path: *const c_char,
        out_width: *mut c_int,
        out_height: *mut c_int,
    ) -> c_int;
}

/// `path` fitted inside `target_size` (never enlarged), turned by its EXIF
/// orientation and colour matched to sRGB, as straight-alpha BGRA8.
pub(crate) fn decode(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let max_size = c_int::try_from(target_size)?;
    let (mut width, mut height, mut data) = (0, 0, std::ptr::null_mut());
    // SAFETY: the path is a valid C string; the out-pointers live for the call.
    let ok = unsafe {
        lumina_image_decode(
            c_path.as_ptr(),
            max_size,
            &mut width,
            &mut height,
            &mut data,
        )
    };
    if ok == 0 || data.is_null() {
        return Err(anyhow!("ImageIO could not decode {}", path.display()));
    }
    let (width, height) = (width as u32, height as u32);
    let len = width as usize * height as usize * 4;
    // SAFETY: on success the bridge returns a malloc'd buffer of exactly
    // width * height * 4 bytes, copied here and then freed once.
    let data = unsafe {
        let bytes = std::slice::from_raw_parts(data, len).to_vec();
        lumina_image_free(data);
        bytes
    };
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

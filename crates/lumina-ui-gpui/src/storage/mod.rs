//! The library volume's capacity for Settings › Library's storage card.
//! macOS reads it through `macos_volume.m`; other platforms report none.

use std::path::Path;

#[cfg(target_os = "macos")]
extern "C" {
    /// Total and available bytes of the volume holding `path`; false when
    /// either cannot be read. Any thread.
    fn lumina_volume_capacity(
        path: *const std::ffi::c_char,
        total: *mut u64,
        available: *mut u64,
    ) -> bool;
}

/// `(total, available)` bytes of the volume holding `path`, or `None` when
/// they cannot be read (or off macOS). Touches the disk: call it off the UI
/// thread.
#[cfg(target_os = "macos")]
pub(crate) fn volume_capacity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let (mut total, mut available) = (0u64, 0u64);
    // SAFETY: `path` is a NUL-terminated string that outlives the call, and
    // both outputs point at live u64s.
    let ok = unsafe { lumina_volume_capacity(path.as_ptr(), &mut total, &mut available) };
    ok.then_some((total, available))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn volume_capacity(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[core::prelude::v1::test]
    fn volume_capacity_of_temp_dir_is_positive() {
        let (total, available) = super::volume_capacity(&std::env::temp_dir())
            .expect("the temp directory's volume reports its capacity");
        assert!(
            total > 0 && available <= total,
            "a volume has a size and never more free space than that: \
             {available} of {total}"
        );
        assert!(
            super::volume_capacity(std::path::Path::new("/nonexistent/lumina")).is_none(),
            "a path that does not exist has no volume to read"
        );
    }
}

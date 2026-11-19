//! Best-effort volume identity for a library root.
//!
//! A library is stored once, with every path relative to its root, so that the
//! same catalog can be recognised when its volume comes back at a different
//! mount point (`/media/u/Photos` today, `/run/media/u/Photos` tomorrow, or a
//! macOS volume mounted as `Photos 1`). The identity is a filesystem UUID
//! where the platform exposes one, the mounted device next, and the mount
//! point last; it is a hint for remapping, and a candidate is only adopted
//! after its contents match the catalog.

use std::path::{Path, PathBuf};

/// What identifies a volume and where it is mounted right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeIdentity {
    /// `uuid:<fs-uuid>`, else `dev:<device>`, else `mount:<path>`.
    pub id: String,
    /// The volume's mount point, as the OS reports it.
    pub mount_point: PathBuf,
}

impl VolumeIdentity {
    /// `root` relative to the volume's mount point; empty when the library is
    /// the mount point itself.
    pub fn rel_from(&self, root: &Path) -> PathBuf {
        root.strip_prefix(&self.mount_point)
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

/// Identifies the volume holding `path`, if the platform can.
pub fn probe(path: &Path) -> Option<VolumeIdentity> {
    #[cfg(target_os = "linux")]
    {
        linux::probe(path)
    }
    #[cfg(target_os = "macos")]
    {
        macos::probe(path)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        None
    }
}

/// Every mount where the volume with `id` appears now. A filesystem UUID can
/// be mounted at several paths (bind mounts, btrfs subvolumes), so callers
/// choose the candidate whose contents match.
pub fn find_mounts(id: &str) -> Vec<PathBuf> {
    // `mount:` ids name their own location (a fallback for platforms that
    // report no id, and for tests); `root:` ids cannot be relocated.
    if let Some(path) = id.strip_prefix("mount:") {
        let path = PathBuf::from(path);
        return if path.is_dir() {
            vec![path]
        } else {
            Vec::new()
        };
    }
    if id.starts_with("root:") {
        return Vec::new();
    }
    #[cfg(target_os = "linux")]
    {
        linux::find_mounts(id)
    }
    #[cfg(target_os = "macos")]
    {
        macos::find_mounts(id)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = id;
        Vec::new()
    }
}

/// The id to store when the platform cannot probe the volume: unique to the
/// root, so it never remaps, but the row stays well-formed.
pub(crate) fn fallback_id(root: &Path) -> String {
    format!("root:{}", root.to_string_lossy())
}

#[cfg(target_os = "macos")]
fn c_path(path: &Path) -> Option<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes()).ok()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    pub(super) fn probe(path: &Path) -> Option<VolumeIdentity> {
        let entries = mount_entries();
        let canonical = path.canonicalize().ok()?;
        let (mount, source) = entries
            .into_iter()
            .filter(|(mount, _)| canonical.starts_with(mount))
            .max_by_key(|(mount, _)| mount.as_os_str().len())?;
        let id = source
            .as_deref()
            .and_then(uuid_for_device)
            .unwrap_or_else(|| {
                let device = std::fs::metadata(path).map(|meta| meta.dev()).unwrap_or(0);
                format!("dev:{device}")
            });
        Some(VolumeIdentity {
            id,
            mount_point: mount,
        })
    }

    pub(super) fn find_mounts(id: &str) -> Vec<PathBuf> {
        mount_entries()
            .into_iter()
            .filter_map(|(mount, source)| {
                let found = match id.strip_prefix("uuid:") {
                    Some(want) => {
                        source.as_deref().and_then(uuid_for_device) == Some(format!("uuid:{want}"))
                    }
                    None => {
                        let want: u64 = id.strip_prefix("dev:")?.parse().ok()?;
                        std::fs::metadata(&mount).is_ok_and(|metadata| metadata.dev() == want)
                    }
                };
                found.then_some(mount)
            })
            .collect()
    }

    /// The filesystem UUID of the device at `source`, as
    /// `/dev/disk/by-uuid` names it.
    fn uuid_for_device(source: &Path) -> Option<String> {
        let device = source.canonicalize().ok()?;
        let entries = std::fs::read_dir("/dev/disk/by-uuid").ok()?;
        for entry in entries.flatten() {
            if entry.path().canonicalize().ok().as_deref() == Some(device.as_path()) {
                return entry
                    .file_name()
                    .to_str()
                    .map(|uuid| format!("uuid:{uuid}"));
            }
        }
        None
    }

    /// `(mount point, source device)` for every mount in `/proc/self/mountinfo`.
    fn mount_entries() -> Vec<(PathBuf, Option<PathBuf>)> {
        let Ok(text) = std::fs::read_to_string("/proc/self/mountinfo") else {
            return Vec::new();
        };
        parse_mountinfo(&text)
    }

    fn parse_mountinfo(text: &str) -> Vec<(PathBuf, Option<PathBuf>)> {
        text.lines()
            .filter_map(|line| {
                let (before, after) = line.split_once(" - ")?;
                let mount = before.split(' ').nth(4)?;
                let source = after.split(' ').nth(2)?;
                let mount = PathBuf::from(unescape(mount.as_bytes()));
                let source = source
                    .starts_with('/')
                    .then(|| PathBuf::from(unescape(source.as_bytes())));
                Some((mount, source))
            })
            .collect()
    }

    /// `/proc` escapes space, tab, newline and backslash as `\ooo`.
    fn unescape(field: &[u8]) -> String {
        let mut out = Vec::with_capacity(field.len());
        let mut i = 0;
        while i < field.len() {
            if field[i] == b'\\' && i + 3 < field.len() {
                if let Ok(value) =
                    u8::from_str_radix(std::str::from_utf8(&field[i + 1..i + 4]).unwrap_or(""), 8)
                {
                    out.push(value);
                    i += 4;
                    continue;
                }
            }
            out.push(field[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mountinfo_parses_escaped_mounts_and_sources() {
            let text =
                "36 35 98:0 /mnt/a\\040b /media/My\\040Drive rw,relatime - ext4 /dev/sdb1 rw\n\
                        37 35 0:33 / / rw,relatime - overlay overlay rw\n";
            let entries = parse_mountinfo(text);
            assert_eq!(entries[0].0, PathBuf::from("/media/My Drive"));
            assert_eq!(entries[0].1, Some(PathBuf::from("/dev/sdb1")));
            assert_eq!(entries[1].0, PathBuf::from("/"));
            assert_eq!(entries[1].1, None, "overlay has no device path");
        }

        #[test]
        fn a_temporary_directory_is_under_its_reported_mount() {
            let dir = std::env::temp_dir();
            let found = probe(&dir).expect("the temp directory has a volume");
            let canonical = dir.canonicalize().expect("temp dir canonicalizes");
            assert!(
                canonical.starts_with(&found.mount_point),
                "{canonical:?} must be under its mount {:?}",
                found.mount_point
            );
            assert_eq!(
                find_mount(&found.id),
                Some(found.mount_point.clone()),
                "the probed volume is found again by its id"
            );
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;

    pub(super) fn probe(path: &Path) -> Option<VolumeIdentity> {
        let (mount_point, source) = statfs(path)?;
        let id = volume_uuid(path)
            .map(|uuid| format!("uuid:{uuid}"))
            .unwrap_or_else(|| format!("dev:{}", source.to_string_lossy()));
        Some(VolumeIdentity { id, mount_point })
    }

    pub(super) fn find_mounts(id: &str) -> Vec<PathBuf> {
        candidate_mounts()
            .into_iter()
            .filter(|mount| probe(mount).is_some_and(|found| found.id == id))
            .collect()
    }

    fn candidate_mounts() -> Vec<PathBuf> {
        let mut mounts = vec![PathBuf::from("/")];
        // Data volumes (the firmlinked home), then external volumes.
        for root in ["/System/Volumes", "/Volumes"] {
            if let Ok(entries) = std::fs::read_dir(root) {
                mounts.extend(entries.flatten().map(|entry| entry.path()));
            }
        }
        mounts
    }

    fn statfs(path: &Path) -> Option<(PathBuf, PathBuf)> {
        let path = c_path(path)?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(path.as_ptr(), &mut stat) } != 0 {
            return None;
        }
        let mount = unsafe { CStr::from_ptr(stat.f_mntonname.as_ptr()) };
        let source = unsafe { CStr::from_ptr(stat.f_mntfromname.as_ptr()) };
        let mount = PathBuf::from(std::ffi::OsStr::from_bytes(mount.to_bytes()));
        let source = PathBuf::from(std::ffi::OsStr::from_bytes(source.to_bytes()));
        Some((mount, source))
    }

    /// The volume's filesystem UUID (`ATTR_VOL_UUID`), which survives a
    /// remount where `/dev/diskN` does not.
    fn volume_uuid(path: &Path) -> Option<String> {
        let path = c_path(path)?;
        let mut attrs: libc::attrlist = unsafe { std::mem::zeroed() };
        attrs.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        attrs.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_UUID;
        let mut buffer = [0u8; 32];
        let rc = unsafe {
            libc::getattrlist(
                path.as_ptr(),
                (&mut attrs) as *mut libc::attrlist as *mut std::ffi::c_void,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                buffer.len(),
                0,
            )
        };
        if rc != 0 {
            return None;
        }
        // [u32 total length][16-byte UUID], with no group-length field for a
        // single volume attribute; `total` is 20 on success.
        let total = u32::from_ne_bytes(buffer[0..4].try_into().ok()?);
        if total < 20 {
            return None;
        }
        let uuid = &buffer[4..20];
        Some(
            uuid.iter()
                .enumerate()
                .map(|(i, byte)| {
                    let dash = matches!(i, 4 | 6 | 8 | 10);
                    format!("{}{byte:02x}", if dash { "-" } else { "" })
                })
                .collect(),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_temporary_directory_is_under_its_reported_mount() {
            let dir = std::env::temp_dir();
            let found = probe(&dir).expect("the temp directory has a volume");
            let canonical = dir.canonicalize().expect("temp dir canonicalizes");
            assert!(
                found.mount_point.is_dir(),
                "{:?} is a real mount point",
                found.mount_point
            );
            // A firmlinked path reports `/System/Volumes/Data`, which is not a
            // prefix; only a real mount boundary can round-trip.
            if canonical.starts_with(&found.mount_point) {
                assert!(
                    find_mounts(&found.id).contains(&found.mount_point),
                    "the probed volume is found again by its id"
                );
            }
        }

        #[test]
        fn the_system_volume_reports_a_uuid() {
            let found = probe(Path::new("/")).expect("the root volume probes");
            let uuid = found
                .id
                .strip_prefix("uuid:")
                .expect("the root volume has a filesystem UUID");
            assert_eq!(uuid.len(), 36, "a canonical UUID, got {uuid}");
            assert!(
                find_mounts(&found.id).contains(&PathBuf::from("/")),
                "the root volume is found again by its UUID"
            );
        }
    }
}

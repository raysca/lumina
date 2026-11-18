//! Downloading, verifying and installing an update on macOS.
//!
//! Trust comes from Apple code signing, as in Electron's macOS updater: the
//! new app must meet the running app's designated requirement (same
//! Developer ID team and bundle id) plus `notarized`, at staging and again
//! just before the swap. The
//! SHA-256 from the release only catches a damaged download; it comes from
//! the same place as the zip, so it proves nothing about who made it.
//!
//! Everything is staged in a hidden `.Lumina-update-<nanos>` folder beside
//! the app, so the final swap is two renames on one volume.

use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{expected_sha256, Release, NO_DOWNLOAD};
use crate::net_import::run_captured;

/// Name prefix of staging folders; `remove_stale` deletes leftovers.
const STAGING_PREFIX: &str = ".Lumina-update-";
/// Name prefix of the current app while it is swapped aside; never removed
/// by `remove_stale`.
const PREVIOUS_PREFIX: &str = ".Lumina-previous-";
/// Longest the zip download may take.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Longest any other step (small download, unzip, signature check) may take.
const STEP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// User-facing refusals. They name no tool; details go to the log.
pub const NOT_IN_APP: &str =
    "Only the Lumina app can update itself. Download the new version from the release page.";
pub const TRANSLOCATED: &str = "Move Lumina to your Applications folder, then check again.";
pub const NOT_WRITABLE: &str =
    "Lumina can't replace itself in this folder. Download the new version from the release page.";
pub const UNSIGNED: &str =
    "This copy of Lumina can't update itself. Download the new version from the release page.";
pub const BAD_DOWNLOAD: &str = "The update didn't download correctly. Try again later.";
pub const UNTRUSTED: &str = "The update couldn't be verified, so it wasn't installed.";
pub const INSTALL_FAILED: &str =
    "Lumina couldn't install the update. Your current version is unchanged.";

/// A downloaded, verified update waiting to be swapped in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedUpdate {
    pub version: String,
    /// The staging folder; removed after the swap.
    dir: PathBuf,
    /// The verified `Lumina.app` inside it.
    app: PathBuf,
    /// The running app's designated requirement, to check `app` again at
    /// install: the staging folder is writable by any process of this user.
    requirement: String,
}

impl StagedUpdate {
    /// A staged update for UI tests, which never install it for real.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_tests(version: &str) -> Self {
        Self {
            version: version.to_string(),
            dir: PathBuf::new(),
            app: PathBuf::new(),
            requirement: String::new(),
        }
    }
}

/// The running app's bundle, or why it can't update itself.
pub fn running_bundle() -> Result<PathBuf> {
    let exe = std::env::current_exe()
        .and_then(fs::canonicalize)
        .context(NOT_IN_APP)?;
    bundle_of(&exe)
}

/// `X.app` from `X.app/Contents/MacOS/<binary>`.
fn bundle_of(exe: &Path) -> Result<PathBuf> {
    let bundle = exe
        .ancestors()
        .nth(3)
        .filter(|b| b.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| anyhow!(NOT_IN_APP))?;
    // Gatekeeper runs a never-moved download from a read-only random path.
    if bundle.to_string_lossy().contains("/AppTranslocation/") {
        bail!(TRANSLOCATED);
    }
    Ok(bundle.to_path_buf())
}

/// The bundle's designated requirement, refused unless it is a Developer ID
/// signature (an ad-hoc one pins a hash no new build can ever match).
fn designated_requirement(bundle: &Path) -> Result<String> {
    let output = run_tool(Command::new("codesign").arg("-d").arg("-r-").arg(bundle))?;
    // The requirement is on stdout; older systems print it on stderr.
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let requirement = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("designated => "))
        .filter(|r| r.contains("anchor apple generic") && r.contains("subject.OU"))
        .map(str::to_string);
    requirement.ok_or_else(|| {
        log::warn!(
            "update: running app is not Developer ID signed: {}",
            text.trim()
        );
        anyhow!(UNSIGNED)
    })
}

/// Downloads `release` beside `bundle`, checks it, and unpacks it. Leaves
/// nothing behind on failure.
pub fn stage(release: &Release, bundle: &Path, cancel: &AtomicBool) -> Result<StagedUpdate> {
    let (Some(zip), Some(sums_url)) = (&release.zip, &release.sums_url) else {
        bail!(NO_DOWNLOAD);
    };
    // Checked first: an unsigned copy can't take any update, so don't download one.
    let requirement = designated_requirement(bundle)?;
    let parent = bundle.parent().ok_or_else(|| anyhow!(NOT_WRITABLE))?;
    remove_stale(parent);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = parent.join(format!("{STAGING_PREFIX}{nanos}"));
    fs::create_dir(&dir).map_err(|err| {
        log::warn!("update: can't create {dir:?}: {err}");
        anyhow!(NOT_WRITABLE)
    })?;
    let staged = stage_in(&dir, release, zip, sums_url, &requirement, cancel);
    if staged.is_err() {
        remove(&dir);
    }
    staged
}

fn stage_in(
    dir: &Path,
    release: &Release,
    zip: &super::Asset,
    sums_url: &str,
    requirement: &str,
    cancel: &AtomicBool,
) -> Result<StagedUpdate> {
    let zip_path = dir.join("update.zip");
    download(&zip.url, &zip_path, cancel, DOWNLOAD_TIMEOUT)?;
    let sums_path = dir.join("SHA256SUMS.txt");
    download(sums_url, &sums_path, cancel, STEP_TIMEOUT)?;

    let sums = fs::read_to_string(&sums_path).context(BAD_DOWNLOAD)?;
    let expected = expected_sha256(&sums, &zip.name).ok_or_else(|| {
        log::warn!("update: {} is not in SHA256SUMS.txt", zip.name);
        anyhow!(BAD_DOWNLOAD)
    })?;
    let output = run_tool(Command::new("shasum").args(["-a", "256"]).arg(&zip_path))?;
    let actual = String::from_utf8_lossy(&output.stdout);
    if actual.split_whitespace().next() != Some(expected.as_str()) {
        log::warn!("update: checksum mismatch for {}", zip.name);
        bail!(BAD_DOWNLOAD);
    }

    let unpacked = dir.join("unpacked");
    let output = run_tool(
        Command::new("ditto")
            .args(["-x", "-k"])
            .arg(&zip_path)
            .arg(&unpacked),
    )?;
    if !output.status.success() {
        log::warn!(
            "update: unzip failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        bail!(BAD_DOWNLOAD);
    }
    let app = unpacked.join("Lumina.app");
    verify(&app, requirement, &release.version)?;
    // The zip isn't needed once unpacked; keep the staging folder small.
    if let Err(err) = fs::remove_file(&zip_path) {
        log::warn!("update: can't remove {zip_path:?}: {err}");
    }
    Ok(StagedUpdate {
        version: release.version.clone(),
        dir: dir.to_path_buf(),
        app,
        requirement: requirement.to_string(),
    })
}

/// Refuses `app` unless it is a real folder (not a link), intact, meets
/// `requirement`, is notarized, and is the version the release says.
fn verify(app: &Path, requirement: &str, version: &str) -> Result<()> {
    if !fs::symlink_metadata(app).is_ok_and(|m| m.is_dir()) {
        log::warn!("update: {app:?} is not a folder");
        bail!(UNTRUSTED);
    }
    // `notarized` in the requirement itself: a Gatekeeper assessment
    // passes everything when the user has turned Gatekeeper off.
    if !meets_requirement(app, &format!("({requirement}) and notarized"))? {
        bail!(UNTRUSTED);
    }
    let output = run_tool(
        Command::new("plutil")
            .args(["-extract", "CFBundleShortVersionString", "raw"])
            .arg(app.join("Contents/Info.plist")),
    )?;
    let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if found != version {
        log::warn!("update: release says {version}, app says {found:?}");
        bail!(UNTRUSTED);
    }
    Ok(())
}

/// Whether `app` is intact and satisfies `requirement` (requirement text,
/// as `codesign -d -r-` prints it after `designated => `).
fn meets_requirement(app: &Path, requirement: &str) -> Result<bool> {
    let output = run_tool(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(format!("-R={requirement}"))
            .arg(app),
    )?;
    if !output.status.success() {
        log::warn!(
            "update: signature check failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.status.success())
}

/// Swaps the staged app in for `bundle`. On failure the current app is
/// put back and stays as it was.
pub fn install(staged: &StagedUpdate, bundle: &Path) -> Result<()> {
    // Checked again: the staged copy may have been changed since staging.
    verify(&staged.app, &staged.requirement, &staged.version).context(INSTALL_FAILED)?;
    // A sibling of the bundle: same folder, so the rename needs no write
    // access inside the bundle. Not a staging name: `remove_stale` must
    // never delete it, since after a failed rollback it is the only copy.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let backup = bundle.with_file_name(format!("{PREVIOUS_PREFIX}{nanos}.app"));
    swap(&staged.app, bundle, &backup)?;
    remove(&backup);
    remove(&staged.dir);
    Ok(())
}

/// `bundle` → `backup`, then `new_app` → `bundle`; undoes the first rename
/// when the second fails.
fn swap(new_app: &Path, bundle: &Path, backup: &Path) -> Result<()> {
    fs::rename(bundle, backup).map_err(|err| {
        log::error!("update: can't move {bundle:?} aside: {err}");
        anyhow!(INSTALL_FAILED)
    })?;
    if let Err(err) = fs::rename(new_app, bundle) {
        log::error!("update: can't move the new app into place: {err}");
        if let Err(err) = fs::rename(backup, bundle) {
            // ponytail: nothing more to try; the old app is at `backup`.
            log::error!("update: can't put {backup:?} back: {err}");
        }
        bail!(INSTALL_FAILED);
    }
    Ok(())
}

/// Opens `bundle` once this process has exited: a detached shell waits for
/// our pid to go away, then hands the app to LaunchServices.
pub fn relaunch_after_exit(bundle: &Path) -> Result<()> {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "while kill -0 \"$0\" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open \"$1\"",
        ])
        .arg(std::process::id().to_string())
        .arg(bundle);
    // Its own process group, untracked, so quitting doesn't stop it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command.spawn().context(INSTALL_FAILED)?;
    Ok(())
}

/// Removes staging folders and old copies a crash or failed swap left in `parent`.
// ponytail: runs when the next update is staged; a launch-time sweep if
// leftovers ever matter between updates.
fn remove_stale(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            remove(&entry.path());
        }
    }
}

fn remove(path: &Path) {
    if let Err(err) = fs::remove_dir_all(path) {
        log::warn!("update: can't remove {path:?}: {err}");
    }
}

fn download(url: &str, target: &Path, cancel: &AtomicBool, timeout: Duration) -> Result<()> {
    let output = run_captured(
        Command::new("curl")
            .args([
                "-sS",
                "-f",
                "-L",
                "--proto",
                "=https",
                "--connect-timeout",
                "30",
            ])
            // Give up on a connection that stalls for a minute.
            .args(["--speed-limit", "1", "--speed-time", "60", "-A", "Lumina"])
            .arg("-o")
            .arg(target)
            .arg("--")
            .arg(url),
        cancel,
        timeout,
    )
    .context(BAD_DOWNLOAD)?;
    if !output.status.success() {
        log::warn!(
            "update download failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        bail!(BAD_DOWNLOAD);
    }
    Ok(())
}

/// Runs a system tool with the step timeout; a failure to start is an
/// untrusted update, never a pass.
fn run_tool(command: &mut Command) -> Result<crate::net_import::ToolOutput> {
    run_captured(command, &AtomicBool::new(false), STEP_TIMEOUT).context(UNTRUSTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina-update-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn fake_app(path: &Path, marker: &str) {
        fs::create_dir_all(path.join("Contents/MacOS")).expect("bundle dirs");
        fs::write(path.join("Contents/marker"), marker).expect("marker");
    }

    fn marker(path: &Path) -> String {
        fs::read_to_string(path.join("Contents/marker")).unwrap_or_default()
    }

    #[test]
    fn bundle_is_found_from_the_executable() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Lumina.app/Contents/MacOS/lumina-ui-gpui"
            ))
            .expect("an app executable has a bundle"),
            PathBuf::from("/Applications/Lumina.app")
        );
        let dev = bundle_of(Path::new("/src/lumina/target/debug/lumina-ui-gpui"));
        assert_eq!(
            dev.unwrap_err().to_string(),
            NOT_IN_APP,
            "a dev build can't update itself"
        );
        let moved = bundle_of(Path::new(
            "/private/var/folders/x/AppTranslocation/ABC/d/Lumina.app/Contents/MacOS/lumina",
        ));
        assert_eq!(
            moved.unwrap_err().to_string(),
            TRANSLOCATED,
            "an app Gatekeeper runs from a temporary copy must be moved first"
        );
    }

    #[test]
    fn swap_replaces_the_app() {
        let dir = temp_dir("swap");
        let bundle = dir.join("Lumina.app");
        let staged = dir.join(".Lumina-update-1");
        fake_app(&bundle, "old");
        fake_app(&staged.join("unpacked/Lumina.app"), "new");
        let update = StagedUpdate {
            version: "9.9.9".into(),
            dir: staged.clone(),
            app: staged.join("unpacked/Lumina.app"),
            requirement: "anchor apple generic".into(),
        };
        // The staged copy is checked again at install; an unsigned one
        // (changed after staging) never goes in.
        let err = install(&update, &bundle).expect_err("an unsigned staged app is refused");
        assert_eq!(err.to_string(), INSTALL_FAILED);
        assert_eq!(
            marker(&bundle),
            "old",
            "a refused update leaves the current app"
        );

        let backup = dir.join(".Lumina-previous-1.app");
        swap(&update.app, &bundle, &backup).expect("the swap succeeds");
        assert_eq!(marker(&bundle), "new", "the new app is installed in place");
        assert_eq!(
            marker(&backup),
            "old",
            "the old app is beside it until removed"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_swap_puts_the_old_app_back() {
        let dir = temp_dir("rollback");
        let bundle = dir.join("Lumina.app");
        fake_app(&bundle, "old");
        let backup = dir.join(".Lumina-previous-1.app");
        let missing = dir.join("gone/Lumina.app");
        let err = swap(&missing, &bundle, &backup).expect_err("a missing new app fails");
        assert_eq!(err.to_string(), INSTALL_FAILED);
        assert_eq!(
            marker(&bundle),
            "old",
            "the current app must be restored when the swap fails"
        );
        assert!(!backup.exists(), "nothing is left at the backup path");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_cleanup_touches_only_update_folders() {
        let dir = temp_dir("stale");
        fake_app(&dir.join("Lumina.app"), "keep");
        fake_app(&dir.join(".Lumina-previous-5.app"), "x");
        fs::create_dir(dir.join(".Lumina-update-6")).unwrap();
        fs::create_dir(dir.join("Other.app")).unwrap();
        remove_stale(&dir);
        let mut left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![".Lumina-previous-5.app", "Lumina.app", "Other.app"],
            "only staging folders go; a copy left by a failed rollback is kept"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ad_hoc_signed_apps_cannot_update() {
        let dir = temp_dir("adhoc");
        let app = dir.join("Lumina.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::copy("/usr/bin/true", app.join("Contents/MacOS/Lumina")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>Lumina</string>\
             <key>CFBundleIdentifier</key><string>test.lumina.adhoc</string>\
             </dict></plist>",
        )
        .unwrap();
        let signed = Command::new("codesign")
            .args(["-s", "-", "--force"])
            .arg(&app)
            .output()
            .expect("codesign runs on macOS");
        assert!(
            signed.status.success(),
            "ad-hoc signing the fixture: {signed:?}"
        );
        let err = designated_requirement(&app).expect_err("ad-hoc signatures are refused");
        assert_eq!(err.to_string(), UNSIGNED);

        // The check really evaluates requirements: the fixture meets its own...
        let own = Command::new("codesign")
            .arg("-d")
            .arg("-r-")
            .arg(&app)
            .output()
            .unwrap();
        let own = String::from_utf8_lossy(&own.stdout);
        let own = own
            .lines()
            // Ad-hoc signatures have an implicit requirement, printed as a comment.
            .find_map(|l| l.trim_start_matches("# ").strip_prefix("designated => "))
            .expect("codesign prints the designated requirement");
        assert!(
            meets_requirement(&app, own).expect("codesign runs"),
            "an app must satisfy its own designated requirement ({own})"
        );
        // ...but an ad-hoc app never passes as an update for a Developer ID one.
        let requirement = "anchor apple generic and identifier \"test.lumina.adhoc\" and \
                           certificate leaf[subject.OU] = \"ABCDE12345\"";
        assert!(
            !meets_requirement(&app, requirement).expect("codesign runs"),
            "an ad-hoc app must not satisfy another team's requirement"
        );
        let err = verify(&app, requirement, "0.0.0").expect_err("verification fails");
        assert_eq!(err.to_string(), UNTRUSTED);
        // It meets its own requirement and the version matches (the fixture
        // has none: ""), so only `notarized` refuses it here.
        let err = verify(&app, own, "").expect_err("an app that isn't notarized is refused");
        assert_eq!(
            err.to_string(),
            UNTRUSTED,
            "notarization is required, not just a matching signature"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

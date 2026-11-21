//! Self-update: finds the newest published release, verifies its signed
//! manifest, downloads the artifact for this platform and installs it (see
//! `docs/updates.md`).
//!
//! The check sends one request to GitHub with no user data. Nothing runs
//! unless the user asks (Check for Updates) or turned on the daily check.

pub mod install;
pub mod manifest;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::net_import::run_captured;

/// The newest published (not draft, not pre-release) release.
const LATEST_URL: &str = "https://api.github.com/repos/raysca/lumina/releases/latest";
/// Longest the release lookup may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest a small download (the manifest, its signature) may take.
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Shown when the lookup fails; the reason goes to the log.
pub const CHECK_FAILED: &str =
    "Lumina couldn't check for updates. Check your connection and try again.";
/// Shown when a release has no download this platform can install.
pub const NO_DOWNLOAD: &str =
    "This update can't be installed automatically. Download it from the release page.";

/// A published release: its files, and the version and page the tag names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `1.2.3`, without the tag's `v`.
    pub version: String,
    /// The release's web page, for a manual download.
    pub page_url: String,
    /// Every HTTPS asset the release attaches, by name.
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }

    /// The signed manifest every platform verifies.
    pub fn manifest(&self) -> Option<&Asset> {
        self.asset(manifest::MANIFEST_NAME)
    }

    pub fn manifest_signature(&self) -> Option<&Asset> {
        self.asset(manifest::SIGNATURE_NAME)
    }
}

/// One downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Asks GitHub for the newest published release.
pub fn fetch_latest(cancel: &AtomicBool) -> Result<Release> {
    let output = run_captured(
        Command::new("curl")
            .args(["-sS", "-f", "-L", "--proto", "=https", "--max-time", "20"])
            .args(["-H", "Accept: application/vnd.github+json"])
            .args(["-A", "Lumina", "--", LATEST_URL]),
        cancel,
        CHECK_TIMEOUT,
    )
    .context(CHECK_FAILED)?;
    if !output.status.success() {
        log::warn!(
            "update check failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Err(anyhow!(CHECK_FAILED));
    }
    parse_release(&String::from_utf8_lossy(&output.stdout))
}

/// Reads GitHub's release JSON. The manifest names the files and their
/// hashes, so every asset is kept for the manifest to point at.
pub fn parse_release(json: &str) -> Result<Release> {
    let release: GhRelease = serde_json::from_str(json).map_err(|err| {
        log::warn!("update check: unexpected release data: {err}");
        anyhow!(CHECK_FAILED)
    })?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    if parse_version(&version).is_none() {
        log::warn!("update check: unexpected tag {:?}", release.tag_name);
        return Err(anyhow!(CHECK_FAILED));
    }
    let assets = release
        .assets
        .into_iter()
        .filter(|asset| asset.browser_download_url.starts_with("https://"))
        .map(|asset| Asset {
            name: asset.name,
            url: asset.browser_download_url,
        })
        .collect();
    Ok(Release {
        version,
        page_url: release.html_url,
        assets,
    })
}

/// Downloads and verifies the release's signed manifest.
pub fn fetch_manifest(release: &Release, cancel: &AtomicBool) -> Result<manifest::UpdateManifest> {
    let (Some(manifest_asset), Some(signature_asset)) =
        (release.manifest(), release.manifest_signature())
    else {
        log::warn!("update: {} has no signed update manifest", release.version);
        bail!(NO_DOWNLOAD);
    };
    let dir = std::env::temp_dir().join(format!(
        "lumina-update-manifest-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).context(NO_DOWNLOAD)?;
    let result = (|| {
        let manifest_path = dir.join(manifest::MANIFEST_NAME);
        let signature_path = dir.join(manifest::SIGNATURE_NAME);
        download(
            &manifest_asset.url,
            &manifest_path,
            cancel,
            MANIFEST_TIMEOUT,
        )?;
        download(
            &signature_asset.url,
            &signature_path,
            cancel,
            MANIFEST_TIMEOUT,
        )?;
        if std::fs::metadata(&manifest_path)
            .map(|m| m.len() > manifest::MAX_MANIFEST_BYTES)
            .unwrap_or(true)
        {
            log::warn!("update: manifest is missing or too large");
            bail!(NO_DOWNLOAD);
        }
        let bytes = std::fs::read(&manifest_path).context(NO_DOWNLOAD)?;
        let signature = std::fs::read_to_string(&signature_path).context(NO_DOWNLOAD)?;
        // Details name the manifest internals; users get the plain sentence.
        let parsed = manifest::verify(&bytes, &signature).map_err(|err| {
            log::warn!("update: {err:#}");
            anyhow!(NO_DOWNLOAD)
        })?;
        if parsed.version != release.version {
            log::warn!(
                "update: manifest says {} but the release is {}",
                parsed.version,
                release.version
            );
            bail!(NO_DOWNLOAD);
        }
        Ok(parsed)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// The macOS zip for `arch` (`std::env::consts::ARCH`): the universal build,
/// else this architecture's.
pub fn macos_artifact<'a>(
    manifest: &'a manifest::UpdateManifest,
    arch: &str,
) -> Option<&'a manifest::UpdateArtifact> {
    let arch = if arch == "aarch64" { "arm64" } else { arch };
    manifest
        .artifact("macos-universal")
        .or_else(|| manifest.artifact(&format!("macos-{arch}")))
}

/// Downloads `url` to `target` with system curl.
pub(crate) fn download(
    url: &str,
    target: &Path,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<()> {
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
    .context(NO_DOWNLOAD)?;
    if !output.status.success() {
        log::warn!(
            "update download failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        bail!(NO_DOWNLOAD);
    }
    Ok(())
}

/// How this Linux copy was installed. macOS updates itself; Linux hands the
/// install to the package manager, which owns `/usr/bin` and the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Flatpak,
    /// A Debian package under `/usr/bin`, installed with apt.
    Deb,
    /// A build we can't classify; offer the release page.
    Unknown,
}

/// The running copy's package kind. Only Linux has one today.
pub fn package_kind() -> PackageKind {
    if !cfg!(target_os = "linux") {
        return PackageKind::Unknown;
    }
    if Path::new("/.flatpak-info").exists() || std::env::var_os("FLATPAK_ID").is_some() {
        return PackageKind::Flatpak;
    }
    let beside_system = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .is_some_and(|dir| dir == Path::new("/usr/bin"));
    if beside_system {
        PackageKind::Deb
    } else {
        PackageKind::Unknown
    }
}

/// What the About page says when a newer release exists but this build must
/// not replace itself.
pub fn managed_message(kind: PackageKind, version: &str) -> String {
    match kind {
        PackageKind::Flatpak => format!(
            "Lumina {version} is available. Update it from Flathub in your software center."
        ),
        PackageKind::Deb => format!(
            "Lumina {version} is available. Install it with your package manager or download the .deb."
        ),
        PackageKind::Unknown => {
            format!("Lumina {version} is available. Download it from the release page.")
        }
    }
}

/// `major.minor.patch`; anything else (a pre-release suffix) is `None`.
pub(crate) fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|p| p.parse::<u64>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

/// Whether `candidate` is a newer version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::manifest::{UpdateArtifact, UpdateManifest};
    use std::collections::BTreeMap;

    const RELEASE: &str = r#"{
        "tag_name": "v0.3.0",
        "html_url": "https://github.com/raysca/lumina/releases/tag/v0.3.0",
        "assets": [
            {"name": "Lumina-v0.3.0.dmg", "browser_download_url": "https://example.test/a.dmg"},
            {"name": "Lumina-v0.3.0-macos-arm64.zip", "browser_download_url": "https://example.test/arm.zip"},
            {"name": "update.json", "browser_download_url": "https://example.test/update.json"},
            {"name": "update.json.sig", "browser_download_url": "https://example.test/update.json.sig"},
            {"name": "insecure.zip", "browser_download_url": "http://example.test/insecure.zip"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://example.test/sums"}
        ]
    }"#;

    fn artifact(name: &str) -> UpdateArtifact {
        UpdateArtifact {
            name: name.to_string(),
            sha256: "a".repeat(64),
        }
    }

    fn manifest(entries: &[(&str, &str)]) -> UpdateManifest {
        UpdateManifest {
            version: "0.3.0".into(),
            page_url: "https://example.test/release".into(),
            artifacts: entries
                .iter()
                .map(|(key, name)| (key.to_string(), artifact(name)))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn release_keeps_https_assets_and_finds_the_manifest() {
        let release = parse_release(RELEASE).expect("valid release JSON parses");
        assert_eq!(release.version, "0.3.0", "the tag's v is dropped");
        assert!(release.asset("Lumina-v0.3.0.dmg").is_some());
        assert!(
            release.asset("insecure.zip").is_none(),
            "an http asset is never used"
        );
        assert_eq!(
            release.manifest().map(|asset| asset.name.as_str()),
            Some("update.json")
        );
        assert_eq!(
            release
                .manifest_signature()
                .map(|asset| asset.name.as_str()),
            Some("update.json.sig")
        );
    }

    #[test]
    fn macos_artifact_prefers_the_universal_zip() {
        let m = manifest(&[
            ("macos-arm64", "Lumina-v0.3.0-macos-arm64.zip"),
            ("macos-universal", "Lumina-v0.3.0-macos-universal.zip"),
        ]);
        assert_eq!(
            macos_artifact(&m, "x86_64").map(|a| a.name.as_str()),
            Some("Lumina-v0.3.0-macos-universal.zip"),
            "the universal build runs on every Mac, so it wins"
        );
    }

    #[test]
    fn macos_artifact_falls_back_to_this_architecture() {
        let m = manifest(&[
            ("macos-arm64", "Lumina-v0.3.0-macos-arm64.zip"),
            ("macos-x86_64", "Lumina-v0.3.0-macos-x86_64.zip"),
        ]);
        assert_eq!(
            macos_artifact(&m, "aarch64").map(|a| a.name.as_str()),
            Some("Lumina-v0.3.0-macos-arm64.zip"),
            "an Apple silicon Mac takes the arm64 zip"
        );
        let none = manifest(&[("macos-x86_64", "Lumina-v0.3.0-macos-x86_64.zip")]);
        assert!(
            macos_artifact(&none, "aarch64").is_none(),
            "no build for this Mac means no automatic install"
        );
    }

    #[test]
    fn release_refuses_odd_tags() {
        let beta = RELEASE.replace("v0.3.0\"", "v0.3.0-beta\"");
        assert!(
            parse_release(&beta).is_err(),
            "a pre-release tag is refused"
        );
        assert!(parse_release("<html>").is_err(), "a web page is refused");
    }

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(
            is_newer("1.0.0", "0.10.0"),
            "numbers compare as numbers, not text"
        );
        assert!(
            !is_newer("0.2.0", "0.2.0"),
            "the same version is not an update"
        );
        assert!(!is_newer("0.1.9", "0.2.0"), "never offer a downgrade");
        assert!(
            !is_newer("0.3.0-beta", "0.2.0"),
            "pre-releases are never offered"
        );
        assert!(!is_newer("0.3", "0.2.0"));
    }

    #[test]
    fn managed_updates_name_the_package_manager() {
        assert!(managed_message(PackageKind::Flatpak, "0.3.0").contains("Flathub"));
        assert!(managed_message(PackageKind::Deb, "0.3.0").contains("package manager"));
        assert!(managed_message(PackageKind::Unknown, "0.3.0").contains("release page"));
    }
}

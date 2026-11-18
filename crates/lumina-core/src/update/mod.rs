//! Self-update: finds the newest published Lumina release, downloads its
//! macOS zip, checks that Apple signed it for the same developer as the
//! running app, and swaps it in (see `docs/updates.md`).
//!
//! The check sends one request to GitHub with no user data. Nothing runs
//! unless the user asks (Check for Updates) or turned on the daily check.

// Builds on every Unix so its tests run in the Linux job; only macOS offers it.
pub mod install;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::net_import::run_captured;

/// The newest published (not draft, not pre-release) release.
const LATEST_URL: &str = "https://api.github.com/repos/raysca/lumina/releases/latest";
/// Longest the release lookup may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// Shown when the lookup fails; the reason goes to the log.
pub const CHECK_FAILED: &str =
    "Lumina couldn't check for updates. Check your connection and try again.";
/// Shown when a release has no download this Mac can install by itself.
pub const NO_DOWNLOAD: &str =
    "This update can't be installed automatically. Download it from the release page.";

/// A published release and the files an update needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `1.2.3`, without the tag's `v`.
    pub version: String,
    /// The release's web page, for a manual download.
    pub page_url: String,
    /// The macOS zip for this Mac, if the release has one.
    pub zip: Option<Asset>,
    /// `SHA256SUMS.txt`, if the release has one.
    pub sums_url: Option<String>,
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
    parse_release(
        &String::from_utf8_lossy(&output.stdout),
        std::env::consts::ARCH,
    )
}

/// Reads GitHub's release JSON, picking the zip for `arch`
/// (`std::env::consts::ARCH`): the universal build, else this architecture's.
pub fn parse_release(json: &str, arch: &str) -> Result<Release> {
    let release: GhRelease = serde_json::from_str(json).map_err(|err| {
        log::warn!("update check: unexpected release data: {err}");
        anyhow!(CHECK_FAILED)
    })?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    if parse_version(&version).is_none() {
        log::warn!("update check: unexpected tag {:?}", release.tag_name);
        return Err(anyhow!(CHECK_FAILED));
    }
    // The packaging script's names: Lumina-v<ver>-macos-<universal|arm64|x86_64>.zip.
    let arch = if arch == "aarch64" { "arm64" } else { arch };
    let zip_for = |suffix: &str| {
        release.assets.iter().find(|a| {
            a.name.starts_with("Lumina-v")
                && a.name.ends_with(&format!("-macos-{suffix}.zip"))
                && a.browser_download_url.starts_with("https://")
        })
    };
    let zip = zip_for("universal")
        .or_else(|| zip_for(arch))
        .map(|a| Asset {
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
        });
    let sums_url = release
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS.txt" && a.browser_download_url.starts_with("https://"))
        .map(|a| a.browser_download_url.clone());
    Ok(Release {
        version,
        page_url: release.html_url,
        zip,
        sums_url,
    })
}

/// `major.minor.patch`; anything else (a pre-release suffix) is `None`.
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
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

/// The SHA-256 that `SHA256SUMS.txt` (`shasum -a 256` output) lists for `name`.
pub fn expected_sha256(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        // `shasum` marks binary mode with a leading `*`.
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "v0.3.0",
        "html_url": "https://github.com/raysca/lumina/releases/tag/v0.3.0",
        "assets": [
            {"name": "Lumina-v0.3.0.dmg", "browser_download_url": "https://example.test/a.dmg"},
            {"name": "Lumina-v0.3.0-macos-arm64.zip", "browser_download_url": "https://example.test/arm.zip"},
            {"name": "Lumina-v0.3.0-macos-universal.zip", "browser_download_url": "https://example.test/uni.zip"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://example.test/sums"}
        ]
    }"#;

    #[test]
    fn release_prefers_the_universal_zip() {
        let release = parse_release(RELEASE, "x86_64").expect("valid release JSON parses");
        assert_eq!(release.version, "0.3.0", "the tag's v is dropped");
        assert_eq!(
            release.zip.map(|z| z.name).as_deref(),
            Some("Lumina-v0.3.0-macos-universal.zip"),
            "the universal build runs on every Mac, so it wins"
        );
        assert_eq!(
            release.sums_url.as_deref(),
            Some("https://example.test/sums")
        );
    }

    #[test]
    fn release_falls_back_to_this_architecture() {
        let json = RELEASE.replace("macos-universal", "macos-x86_64");
        let arm = parse_release(&json, "aarch64").expect("parses");
        assert_eq!(
            arm.zip.map(|z| z.url).as_deref(),
            Some("https://example.test/arm.zip"),
            "an Apple silicon Mac takes the arm64 zip"
        );
        let json = json.replace("macos-arm64", "macos-ppc");
        let none = parse_release(&json, "aarch64").expect("parses");
        assert_eq!(
            none.zip, None,
            "no zip for this Mac means no automatic install"
        );
    }

    #[test]
    fn release_refuses_plain_http_and_odd_tags() {
        let http = RELEASE.replace(
            "https://example.test/uni.zip",
            "http://example.test/uni.zip",
        );
        let release = parse_release(&http, "x86_64").expect("parses");
        assert_eq!(
            release.zip, None,
            "a download that isn't HTTPS is never used (arm64 doesn't fit x86_64 either)"
        );
        let beta = RELEASE.replace("v0.3.0\"", "v0.3.0-beta\"");
        assert!(
            parse_release(&beta, "x86_64").is_err(),
            "a pre-release tag is refused"
        );
        assert!(
            parse_release("<html>", "x86_64").is_err(),
            "a web page is refused"
        );
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
    fn sums_lookup_matches_the_exact_file() {
        let sums = "ABC123  Lumina-v0.3.0-macos-universal.zip\n\
                    def456 *Lumina-v0.3.0.dmg\n";
        assert_eq!(
            expected_sha256(sums, "Lumina-v0.3.0-macos-universal.zip").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            expected_sha256(sums, "Lumina-v0.3.0.dmg").as_deref(),
            Some("def456")
        );
        assert_eq!(expected_sha256(sums, "Lumina-v0.3.0.zip"), None);
    }
}

//! The signed update manifest that every platform verifies.
//!
//! A release publishes `update.json` — the version, the release page and, for
//! each platform, an artifact name and its SHA-256 — plus `update.json.sig`,
//! a detached Ed25519 signature from the project's release key. The public
//! half is compiled in ([`UPDATE_PUBLIC_KEY`]); the private half lives only in
//! the release secret store.
//!
//! Code signing still proves who built an artifact, but notarization and a
//! Developer ID cannot cover every build (a source build has neither), so the
//! manifest is the trust anchor for updates: a release that is not notarized
//! can still update, and the same check works on Linux and Windows.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// The manifest asset name a release publishes.
pub const MANIFEST_NAME: &str = "update.json";
/// The detached signature asset name.
pub const SIGNATURE_NAME: &str = "update.json.sig";
/// A manifest is tiny; anything larger is not one.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Base64 of the 32-byte Ed25519 update public key. The matching private key
/// is the `UPDATE_SIGNING_KEY` release secret. To rotate, ship a build that
/// trusts both keys, then drop the old one.
pub const UPDATE_PUBLIC_KEY: &str = "URenHb8vFW2NrxEcKPyTuQh+Ve1xEYvo2PmaGVEz9b4=";

/// What a release says about itself and its files.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateManifest {
    /// `major.minor.patch`, matching the release tag.
    pub version: String,
    /// The release's web page, for a manual download.
    pub page_url: String,
    /// Platform key (`macos-universal`, `linux-x86_64-deb`, …) to artifact.
    #[serde(default)]
    pub artifacts: BTreeMap<String, UpdateArtifact>,
}

/// One file a platform can install, named as the release attaches it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateArtifact {
    pub name: String,
    /// Lowercase hex, 64 characters.
    pub sha256: String,
}

impl UpdateManifest {
    pub fn artifact(&self, key: &str) -> Option<&UpdateArtifact> {
        self.artifacts.get(key)
    }
}

/// Verifies `signature` (base64) over `bytes` with the compiled-in key, then
/// parses and validates the manifest.
pub fn verify(bytes: &[u8], signature: &str) -> Result<UpdateManifest> {
    verify_with_key(bytes, signature, UPDATE_PUBLIC_KEY)?;
    parse(bytes)
}

/// The production entry point's split, so tests can use a throwaway key.
pub(crate) fn verify_with_key(bytes: &[u8], signature: &str, key_b64: &str) -> Result<()> {
    let key = base64::engine::general_purpose::STANDARD
        .decode(key_b64.trim())
        .context("update manifest: the public key is not base64")?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature.trim())
        .context("update manifest: the signature is not base64")?;
    if key.len() != 32 {
        bail!("update manifest: the public key is not 32 bytes");
    }
    if signature.len() != 64 {
        bail!("update manifest: the signature is not 64 bytes");
    }
    UnparsedPublicKey::new(&ED25519, &key)
        .verify(bytes, &signature)
        .map_err(|_| anyhow!("update manifest: signature does not match"))
}

/// Parses and validates a manifest. A malformed one is a failed check, not a
/// reason to install something unchecked.
pub fn parse(bytes: &[u8]) -> Result<UpdateManifest> {
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        bail!("update manifest: too large");
    }
    let manifest: UpdateManifest =
        serde_json::from_slice(bytes).context("update manifest: unexpected data")?;
    crate::update::parse_version(&manifest.version)
        .ok_or_else(|| anyhow!("update manifest: version {:?}", manifest.version))?;
    if !manifest.page_url.starts_with("https://") {
        bail!("update manifest: page is not https");
    }
    for (key, artifact) in &manifest.artifacts {
        if !is_platform_key(key) {
            bail!("update manifest: bad platform key {key:?}");
        }
        if !is_plain_name(&artifact.name) {
            bail!("update manifest: bad artifact name {:?}", artifact.name);
        }
        if artifact.sha256.len() != 64
            || !artifact
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            bail!("update manifest: bad sha256 for {key}");
        }
    }
    Ok(manifest)
}

/// Platform keys are ASCII words joined by `-`: `macos-universal`,
/// `linux-x86_64-deb`, `windows-x86_64-msix`.
fn is_platform_key(key: &str) -> bool {
    !key.is_empty()
        && key.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
        })
}

/// A file name, never a path: the download target is built from this.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

/// The SHA-256 of `path`, lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut reader = BufReader::new(File::open(path).context("update: can't read the download")?);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .context("update: can't read the download")?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// RFC 8032 §7.1, test 1: the empty message.
    const TEST_KEY: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const TEST_SIG: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

    fn key_b64() -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(hex(TEST_KEY))
    }

    fn sig_b64() -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(hex(TEST_SIG))
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn manifest(version: &str) -> String {
        format!(
            r#"{{
                "version": "{version}",
                "page_url": "https://github.com/raysca/lumina/releases/tag/v{version}",
                "artifacts": {{
                    "macos-universal": {{ "name": "Lumina-v{version}-macos-universal.zip", "sha256": "{}" }},
                    "linux-x86_64-deb": {{ "name": "Lumina-v{version}-linux-amd64.deb", "sha256": "{}" }}
                }}
            }}"#,
            "a".repeat(64),
            "b".repeat(64)
        )
    }

    #[test]
    fn a_valid_signature_verifies_and_parses() {
        verify_with_key(b"", &sig_b64(), &key_b64()).expect("the RFC 8032 vector verifies");
        let parsed = parse(manifest("0.3.0").as_bytes()).expect("a valid manifest parses");
        assert_eq!(parsed.version, "0.3.0");
        assert_eq!(
            parsed.artifact("macos-universal").map(|a| a.name.as_str()),
            Some("Lumina-v0.3.0-macos-universal.zip")
        );
    }

    #[test]
    fn tampered_bytes_or_signature_are_rejected() {
        verify_with_key(b"x", &sig_b64(), &key_b64()).expect_err("a changed message fails");
        let wrong = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
        verify_with_key(b"", &wrong, &key_b64()).expect_err("a changed signature fails");
        verify_with_key(b"", &sig_b64(), "not base64").expect_err("a bad key fails");
    }

    #[test]
    fn malformed_manifests_are_refused() {
        for bad in [
            r#"{"version":"0.3.0","page_url":"http://x","artifacts":{}}"#,
            r#"{"version":"0.3.0-beta","page_url":"https://x","artifacts":{}}"#,
            r#"{"version":"0.3.0","page_url":"https://x","artifacts":{"macos":{"name":"../x","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}"#,
            r#"{"version":"0.3.0","page_url":"https://x","artifacts":{"macos":{"name":"x.zip","sha256":"ABC"}}}"#,
        ] {
            parse(bad.as_bytes()).expect_err("a malformed manifest is refused");
        }
    }

    #[test]
    fn sha256_matches_a_known_digest() {
        let dir = std::env::temp_dir().join(format!("lumina-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("abc.txt");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"abc")
            .unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

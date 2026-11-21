//! Writes the signed update manifest's contents for a release.
//!
//! Usage:
//!   make_update_manifest <out.json> <version> <page-url> <platform=path>...
//!
//! The release workflow passes the built artifacts; the manifest is signed
//! separately with `scripts/sign_update_manifest.sh`.

use lumina_core::update::manifest::{self, UpdateArtifact, UpdateManifest};
use std::collections::BTreeMap;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing output path"))?;
    let version = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing version"))?;
    let page_url = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing page URL"))?;
    let mut artifacts = BTreeMap::new();
    for spec in args {
        let (platform, path) = spec
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("artifact must be <platform>=<path>: {spec:?}"))?;
        let file = Path::new(path);
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("artifact has no file name: {path:?}"))?
            .to_string();
        let sha256 = manifest::sha256_file(file)?;
        artifacts.insert(platform.to_string(), UpdateArtifact { name, sha256 });
    }
    let manifest = UpdateManifest {
        version,
        page_url,
        artifacts,
    };
    // Round-trip through the parser so an invalid manifest fails the release,
    // not the update check.
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    manifest::parse(&bytes)?;
    std::fs::write(&out, bytes)?;
    println!("wrote {out}");
    Ok(())
}

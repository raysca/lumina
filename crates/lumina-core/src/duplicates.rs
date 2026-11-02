use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{MediaId, MediaItem};

/// Computes the 128-bit xxHash (XXH3) of a file's contents, returning a 32-character hex string.
pub fn compute_file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("reading {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.digest128();
    Ok(format!("{:032x}", digest))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuplicateGroup {
    pub hash: String,
    pub file_size: u64,
    pub items: Vec<MediaItem>,
    pub potential_reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DuplicateCandidate {
    pub path: PathBuf,
    pub file_size: u64,
    pub content_hash: Option<String>,
    pub matched_item_id: Option<MediaId>,
    pub matched_item_path: Option<PathBuf>,
    pub is_duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DuplicateReport {
    pub total_scanned: usize,
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicate_items: usize,
    pub potential_reclaimable_bytes: u64,
}

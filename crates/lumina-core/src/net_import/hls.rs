//! HLS (.m3u8) inspection. Downloads go through yt-dlp (see `helper`).

use anyhow::{bail, Result};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use super::direct::curl_error;
use super::{
    run_captured, sanitize_filename, NetworkMediaInfo, NetworkSourceKind, INSPECT_TIMEOUT,
};

fn fetch(url: &str, cancel: &AtomicBool) -> Result<String> {
    let output = run_captured(
        Command::new("curl")
            .args(["-sS", "-f", "-L", "--max-time", "15", "--"])
            .arg(url),
        cancel,
        INSPECT_TIMEOUT,
    )?;
    if !output.status.success() {
        bail!(curl_error(output.status, &output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The first variant playlist a master playlist lists, if it is one.
fn first_variant(body: &str) -> Option<&str> {
    let mut lines = body.lines().map(str::trim);
    lines.find(|l| l.starts_with("#EXT-X-STREAM-INF"))?;
    lines.find(|l| !l.is_empty() && !l.starts_with('#'))
}

/// Resolves a playlist URI against the playlist that referenced it.
fn resolve(base: &str, uri: &str) -> String {
    if uri.contains("://") {
        return uri.to_string();
    }
    let base = base.split(['?', '#']).next().unwrap_or(base);
    if let Some(rest) = uri.strip_prefix('/') {
        // Origin: scheme://host
        let origin_end = base
            .find("://")
            .and_then(|i| base[i + 3..].find('/').map(|j| i + 3 + j))
            .unwrap_or(base.len());
        return format!("{}/{rest}", &base[..origin_end]);
    }
    match base.rfind('/') {
        Some(i) => format!("{}{uri}", &base[..=i]),
        None => uri.to_string(),
    }
}

/// Inspects an HLS playlist; refuses live streams, which never end. A master
/// playlist is judged by its first variant.
pub fn inspect_hls_url(url: &str, cancel: &AtomicBool) -> Result<NetworkMediaInfo> {
    let body = fetch(url, cancel)?;
    if !body.contains("#EXTM3U") {
        bail!("This link doesn't lead to a video Lumina can import.");
    }
    let media = match first_variant(&body) {
        Some(variant) => fetch(&resolve(url, variant), cancel)?,
        None => body,
    };
    if !media.contains("#EXT-X-ENDLIST") {
        bail!("This is a live broadcast. Only recorded videos can be imported.");
    }

    let path = url.split(['?', '#']).next().unwrap_or(url);
    let slug = path.rsplit('/').next().unwrap_or("stream");
    let stem = slug.rsplit_once('.').map(|(base, _)| base).unwrap_or(slug);
    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::HlsStream,
        title: sanitize_filename(stem),
        duration_secs: None,
        uploader: None,
        expected_bytes: None,
        extension: "mp4",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_handles_relative_absolute_and_full_uris() {
        let base = "https://cdn.example.com/a/b/master.m3u8?token=1";
        assert_eq!(
            resolve(base, "v1/index.m3u8"),
            "https://cdn.example.com/a/b/v1/index.m3u8"
        );
        assert_eq!(
            resolve(base, "/x/index.m3u8"),
            "https://cdn.example.com/x/index.m3u8"
        );
        assert_eq!(
            resolve(base, "https://o.example/i.m3u8"),
            "https://o.example/i.m3u8"
        );
    }

    /// A master playlist used to count as on-demand even when its variants
    /// were live.
    #[test]
    fn master_playlist_is_judged_by_its_variant() {
        let dir = std::env::temp_dir().join(format!("lumina-hls-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("master.m3u8"),
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nlive.m3u8\n",
        )
        .unwrap();
        fs::write(dir.join("live.m3u8"), "#EXTM3U\n#EXTINF:6,\nseg0.ts\n").unwrap();
        fs::write(
            dir.join("vod_master.m3u8"),
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nvod.m3u8\n",
        )
        .unwrap();
        fs::write(
            dir.join("vod.m3u8"),
            "#EXTM3U\n#EXTINF:6,\nseg0.ts\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        let cancel = AtomicBool::new(false);
        let base = format!("file://{}", dir.display());

        let live = inspect_hls_url(&format!("{base}/master.m3u8"), &cancel);
        assert!(
            live.unwrap_err().to_string().contains("live broadcast"),
            "a master playlist of a live stream must be refused"
        );
        let vod = inspect_hls_url(&format!("{base}/vod_master.m3u8"), &cancel).unwrap();
        assert_eq!(vod.title, "vod_master");
        fs::remove_dir_all(&dir).unwrap();
    }
}

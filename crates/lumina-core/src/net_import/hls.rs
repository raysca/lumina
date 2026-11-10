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

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::HlsStream,
        title: stream_title(url),
        duration_secs: None,
        uploader: None,
        expected_bytes: None,
        extension: "mp4",
    })
}

/// Playlist names that say nothing about the video.
const GENERIC_NAMES: &[&str] = &[
    "index",
    "master",
    "playlist",
    "manifest",
    "prog_index",
    "main",
    "stream",
    "video",
    "media",
    "chunklist",
    "hls",
    "output",
    "live",
    "vod",
];

/// A stream has no page title, so it is named after its URL: the playlist's
/// file name, or when that is generic ("index", "master", "1080p"…) the
/// nearest meaningful folder above it, then the site's address.
fn stream_title(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let (authority, path) = path
        .split_once("://")
        .map(|(_, rest)| rest.split_once('/').unwrap_or((rest, "")))
        .unwrap_or(("", path));
    // Never `user:password@`, and no `:port`.
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.split(':').next().unwrap_or(""),
    };
    let host = host.strip_prefix("www.").unwrap_or(host);
    let meaningful = |stem: &String| {
        let lower = stem.trim().to_ascii_lowercase();
        // "720p", "1080p": a rendition, not a name.
        let rendition = lower
            .strip_suffix('p')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        !lower.trim_matches('.').is_empty()
            && !rendition
            && !GENERIC_NAMES.contains(&lower.as_str())
    };
    let name = path
        .rsplit('/')
        .map(|segment| {
            let segment = percent_decode(segment);
            match segment.rsplit_once('.') {
                Some((base, _)) if !base.is_empty() => base.to_string(),
                _ => segment,
            }
        })
        .find(meaningful)
        .or_else(|| Some(host.to_string()).filter(|h| !h.is_empty()))
        .unwrap_or_else(|| "stream".to_string());
    sanitize_filename(&name)
}

/// "Big%20Day" -> "Big Day". Invalid escapes stay as they are.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn generic_playlist_names_fall_back_to_the_url() {
        let cases = [
            ("https://cdn.example.com/shows/Big%20Day.m3u8", "Big Day"),
            ("https://cdn.example.com/caf%C3%A9/index.m3u8", "café"),
            ("https://cdn.example.com/a%2/index.m3u8", "a%2"),
            (
                "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
                "x36xhzz",
            ),
            (
                "https://cdn.example.com/films/sunset/index.m3u8?t=1",
                "sunset",
            ),
            (
                "https://cdn.example.com/films/sunset/hls/1080p/master.m3u8",
                "sunset",
            ),
            ("https://www.example.com/index.m3u8", "example.com"),
            ("https://cdn.example.com/hls/Master.M3U8", "cdn.example.com"),
            (
                "https://user:secret@cdn.example.com:8443/index.m3u8",
                "cdn.example.com",
            ),
            ("https://[::1]:8080/index.m3u8", "__1"),
            ("https://example.com", "example.com"),
            ("https://cdn.example.com/films/%69ndex.m3u8", "films"),
            ("https://cdn.example.com/films/%20/index.m3u8", "films"),
            ("https://cdn.example.com/a%+A/index.m3u8", "a%+A"),
            ("file:///index.m3u8", "stream"),
        ];
        for (url, want) in cases {
            assert_eq!(
                stream_title(url),
                want,
                "{url}: a stream is named after the meaningful part of its URL"
            );
        }
    }

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

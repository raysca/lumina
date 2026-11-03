# Import Video from URL

File > Import Video from URL… (⌥⌘U, or "video url" in the command palette)
downloads one video into the library.

## What it accepts

| Link | How it is downloaded |
|---|---|
| Direct file (`.mp4`, `.mov`, `.m4v`, `.mkv`, `.avi`) | system `curl` |
| YouTube and other video sites | bundled yt-dlp |
| HLS playlist (`.m3u8`, recorded streams only) | bundled yt-dlp, remuxed to MP4 by bundled ffmpeg |

Live streams, web pages behind a video link, and formats Lumina can't
catalogue (e.g. WebM) are refused with a message.

## Flow

1. Paste a link (⌘V) and press ↵ (Continue). Lumina checks it: title,
   duration, uploader, size, and whether the same link was imported before.
2. Review the file name, optionally press F for Favorites, and press ↵.
3. The download stages in a hidden `.…lumina-import` folder inside
   `Imports/YYYY/YYYY-MM-DD/`, then takes a free name there (an existing file
   or catalogued item is never replaced; the new one gets " 2"). It is
   catalogued with its source URL and uploader, and a `.xmp` sidecar records
   them as `dc:source` and `photoshop:Credit`. Get Info shows the source.

While downloading, Cancel Download stops at once; Esc asks once, then stops;
clicks outside the dialog are ignored. A crash leaves only the hidden staging
folder, which the next import removes.

## User-facing copy

The dialog and its errors never name the tools behind a download (yt-dlp,
ffmpeg, curl, Homebrew) or stream formats (HLS, m3u8). Messages say what
happened and what the user can do ("This video is private.", "Reinstall
Lumina to fix this."); tool output and full error chains go to the log.

## Privacy and network

- Requests go to the site in the pasted URL and the servers it sends the
  video from (redirects, CDN and API hosts). No user data is sent, nothing runs in the background, and
  no analytics are collected.
- yt-dlp and ffmpeg ship inside the app, pinned by version and SHA-256
  (`scripts/fetch_yt_dlp.sh`, `scripts/fetch_ffmpeg.sh`). Lumina never
  downloads executables at run time. Without the bundled copies (a local
  build), Homebrew's `yt-dlp` and `ffmpeg` are used if installed. yt-dlp and curl run in their own process group with the
  user's yt-dlp config ignored; cancel and quit stop the whole group.
- ffmpeg is built from the pinned source as LGPL-2.1 only, copy-only
  (demuxers, muxers and parsers; no encoders, no network protocols). yt-dlp
  uses it to merge separate video and audio into MP4 and to turn streams into
  MP4 without re-encoding. Downloads prefer the best H.264 version (many Macs
  can't play AV1, and macOS doesn't play VP9 in MP4). If it is missing (a damaged install),
  streams show "Reinstall Lumina" and web video falls back to single-file
  formats.
- Downloading material may be subject to the site's terms and copyright; the
  user is responsible for what they import.

## Development

`make bundle-deps` fetches the pinned yt-dlp, builds the pinned ffmpeg (a few
minutes the first time), and copies both next to the debug and release
binaries. Tests never touch the network: `LUMINA_YT_DLP` points
the lookup at a fake script or a missing path, and direct downloads are tested
against `file://` URLs.

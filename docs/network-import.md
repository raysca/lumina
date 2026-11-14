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

1. Paste a link (⌘V) and press ↵ (Import). Lumina checks whether that link
   was downloaded before and is still in the library; if so it says when and
   offers Show in Library (↵) or Download Again (⌘↵). Otherwise it inspects
   the link (the same request the download would make first) and shows a
   preview: title, site and uploader, length, container and size when the
   site reports them. No thumbnail is fetched. Esc stops the check. A link
   that can't be downloaded shows its message under the field, and nothing is
   queued. Import (↵) on the preview closes the dialog and the link joins the
   download list; the download reuses that inspection.
2. Up to three downloads run at once; the rest wait in the order they were
   added and move up as others finish or are removed.
3. Each download stages in a hidden `.…lumina-import` folder inside
   `Imports/YYYY/YYYY-MM-DD/`, then takes a free name there (an existing file
   or catalogued item is never replaced; the new one gets " 2"). Web videos
   are named after their title. A stream has no title, so it is named after
   its URL: the playlist's file name, or when that is generic (`index`,
   `master`, `1080p`…) the nearest meaningful folder above it, then the
   site's address. It is
   catalogued with its source URL and uploader, and a hidden `.xmp` sidecar records
   them as `dc:source` and `photoshop:Credit`. Get Info shows the source.

## The download list

A toolbar icon appears once the library has any downloads: a count of the
ones downloading, waiting or paused, a ring for their combined progress, and
a red dot when one failed since the list was last opened. ⌥⌘L, View > Show
Downloads and the command palette open it too. The list groups rows into
Downloading, Waiting, Paused, Failed and Done. In it, ↑/↓ move, ↵ retries a
failed row, resumes a paused one or shows a finished one in the library (⌘↵
downloads an "already in your library" row again), ⌫ cancels or removes a
row, and Esc closes it. Finished rows stay until Clear finished. Pasting a
link already in the list highlights its row instead of adding it again.

Quitting or opening another library while downloads run asks first.
Confirming stops them; the next time the library opens they come back
Paused and nothing downloads until Resume or Resume all, which start over
rather than continue. A crash leaves only the hidden staging folder, which
is removed the next time the library opens.

The Downloaded album in the sidebar lists everything downloaded from a link
(not items whose source only came from an imported sidecar). It shows once
it has something in it.

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

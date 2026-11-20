# Linux release QA

Linux is **not supported** until the acceptance criteria in
[`RELEASING.md`](../RELEASING.md#acceptance-criteria-before-calling-windowslinux-supported)
pass on real desktops. This page is the manual pass; copy it into the release
PR or issue, fill it in on each session, and keep the result with the release.

`Xvfb` smokes in CI prove the window opens and a fixture library renders. They
do **not** replace this page: they cannot see audio output, a real compositor,
portal file dialogs, drag and drop, or a removable drive.

## Automated coverage (necessary, not sufficient)

| Check | Where |
| --- | --- |
| fmt, clippy, workspace tests, X11 and Wayland smokes | `.github/workflows/ci.yml` `linux` |
| `.deb` builds, installs, launches and is removed | `ci.yml` `package-linux`, `scripts/check_linux_package.sh` |
| Release `.deb` is versioned, checksummed and signed | `.github/workflows/release.yml` `linux` |
| Flatpak builds with ffmpeg, curl and yt-dlp inside | `.github/workflows/flatpak.yml` |
| AppStream and desktop-entry validation | `scripts/package_linux.sh`, `check_linux_package.sh` |

## Sign-off

| Field | Value |
| --- | --- |
| Version / tag | |
| Tester and date | |
| Verdict | **Not run** / pass with limitations / fail |

## Session matrix

Run the checklist once per session; at minimum the first three.

| Session | Required | Result |
| --- | --- | --- |
| Ubuntu 24.04, GNOME, Wayland | yes | Not run |
| Ubuntu 24.04, GNOME, X11 | yes | Not run |
| Ubuntu 24.04, KDE Plasma (Wayland or X11) | yes | Not run |
| Flatpak build on the sessions above | before Flathub | Not run |
| Library on a removable drive | yes | Not run |
| Second monitor, display scale other than 100% | yes | Not run |

## Install and lifecycle

- [ ] `sudo apt install ./Lumina-vX.Y.Z-linux-amd64.deb` succeeds on a clean
      Ubuntu 24.04 (glibc 2.39+).
- [ ] The app menu entry is named **Lumina** with the correct icon, and it
      launches the app.
- [ ] `sudo apt remove lumina` removes the binary, desktop entry, icon and
      metainfo; the user's libraries, catalog, config and caches stay.
- [ ] Reinstalling or upgrading over an existing install keeps config, catalog,
      window bounds and library list.
- [ ] Uninstall leaves no stray files under `/usr`, `/etc` or `/var`.
- [ ] `yt-dlp` is pulled in by a normal install; with
      `--no-install-recommends`, URL import explains what is missing.

## First run and libraries

- [ ] Welcome screen: Open Existing, Import into New, Explore Sample, Pictures.
- [ ] Scan a 5k+ item library; thumbnails fill in without freezing the UI.
- [ ] Library on a removable drive: open, eject, reconnect, rescan.
- [ ] Remount the drive at a different path (volume remap): the library stays
      usable or says what to do.
- [ ] Settings: add a library, remove one (catalog and thumbnail cache go, the
      photos stay), switch libraries.
- [ ] Quit during a scan or import: no partial catalog state on relaunch.
- [ ] Paths follow XDG: config in `~/.config/lumina`, cache in
      `~/.cache/lumina`, sample library in `~/.local/share/lumina`.

## Grid, viewer, metadata

- [ ] Grid: densities, sort, search, year/album/tag filters, scroll.
- [ ] Viewer: open, previous/next, zoom and pan, 1:1, compare, slideshow.
- [ ] Tags, ratings, labels, captions, favorites, pick/reject, undo/redo.
- [ ] XMP sidecars written and read back; Remove Metadata.
- [ ] Edit in external editor; Edit a Copy; in-place save picked up.
- [ ] Delete to the desktop trash, restore, empty trash; no file lost.
- [ ] Albums and smart albums: create, edit, count, membership, delete/undo.

## Video

- [ ] Poster and scrub previews decode (bundled ffmpeg or the package's).
- [ ] Playback with audio through PulseAudio/PipeWire; mute and volume.
- [ ] Frame stepping, speed, loop and loop range, chapters.
- [ ] Picture-in-Picture in its own window, including on a second display.
- [ ] Audio-track menu when a file has several tracks.
- [ ] A variable-frame-rate clip steps approximately (known limitation).
- [ ] Missing codec: a clear message, no crash or hang.

## URL import

- [ ] Direct `.mp4` link downloads, is catalogued in `Imports/`, and plays.
- [ ] A YouTube link shows the preview card and downloads.
- [ ] An HLS `.m3u8` stream is remuxed and plays.
- [ ] Cancel mid-download; the queue row settles and the partial file is gone.
- [ ] Retry a failed row; duplicate detection offers the earlier download.
- [ ] Flatpak: the same flows work with no host tools visible.

## Desktop integration

- [ ] All menus open and their actions work (File, View, Image, and the rest).
- [ ] Shortcut labels and modifier keys are the Linux ones, and they work.
- [ ] Native file dialogs: open library, import folders, external editor.
- [ ] Drag and drop files and folders onto the window.
- [ ] Keyboard focus, IME, dead keys and composed characters in every field.
- [ ] Window controls, minimum size, fullscreen, and scaling at 150%/200%.
- [ ] Paste a URL with the keyboard; no clipboard surprises.

## Performance and data safety

- [ ] Startup, scrolling and thumbnail throughput are acceptable on a large
      library (record the numbers in the release notes).
- [ ] A background scan does not block the UI or the viewer.
- [ ] Logs are written under the documented location and name no secrets.

## Docs

- [ ] The supported-format table matches what actually renders.
- [ ] Install, upgrade and uninstall instructions work as written.
- [ ] Known limitations are stated: HEIC/HEIF and RAW previews, wide-gamut ICC,
      approximate VFR stepping, Linux arm64 not built.

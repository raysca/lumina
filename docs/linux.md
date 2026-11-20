# Linux development status

Linux support is in development and is not a supported release. CI's
`package-linux` job builds an unsigned `.deb` on pushes to main and on pull
requests and keeps it as a 14-day workflow artifact for internal verification
only; it is never attached to a release. The release checklist in
[`RELEASING.md`](../RELEASING.md#windows-planned-linux-in-development) lists
the remaining acceptance work.

## Current test environment

The Linux CI workflow targets Ubuntu 24.04 x86_64. The code has also been
built and exercised in an Ubuntu arm64 container. Local x86_64 Docker checks on
arm64 hosts run under emulation and do not replace native x86_64 CI. The
internal amd64 `.deb` is built on Ubuntu 24.04 and declares `libc6 (>= 2.39)`.
Ubuntu 24.04 is the minimum tested package environment; other distributions
with glibc 2.39 or newer have not been validated.

For a Linux source build, install the system development packages listed in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml), then run:

```sh
cargo build --workspace
cargo test --workspace
xvfb-run -a ./scripts/live_ui_smoke.sh
xvfb-run -a ./scripts/wayland_smoke.sh
```

The X11 smoke test uses a temporary home and fixture library. It verifies that
the window opens, scans one H.264 video, displays a viewer frame, loads the
GStreamer player, accepts the `F` favorite shortcut through X11, persists the
change to the isolated catalog, and shuts down. The Wayland smoke starts Weston
on a nested X11 display and checks that Lumina creates a Wayland window and
exits when asked.

## Media support

Linux uses GStreamer for video playback and ffmpeg for video posters and scrub
previews. The Debian package declares the GStreamer base, good and libav plugin
packages plus ffmpeg as runtime dependencies. JPEG, PNG, WebP and TIFF previews
are available. HEIC/HEIF and RAW previews are unavailable. Linux does not
currently match wide-gamut ICC profiles to sRGB. Frame stepping uses the
stream's nominal frame rate, so variable-frame-rate clips can step approximately
rather than to the adjacent presentation timestamp. See the [supported formats
in the README](../README.md#supported-formats) for the platform comparison.

## Internal Debian package

Build the package with `./scripts/package_linux.sh`. It writes the `.deb` and
`SHA256SUMS.txt` under `target/release/bundle/linux/`. On a compatible Debian
system, install it with:

```sh
sudo apt install ./target/release/bundle/linux/lumina_*.deb
```

The package installs the application as `lumina`, with the AppStream app id
`io.github.raysca.Lumina`: a desktop entry, a 512 px hicolor icon and an
AppStream metainfo file under `/usr/share`. The bundled font and icon licenses
ship in `/usr/share/doc/lumina/licenses/`. The package script validates the
desktop entry and metainfo (`desktop-file-validate`, `appstreamcli validate`)
before building, and the install check validates them again.

Video playback needs nothing extra: the package depends on the GStreamer
plugins and ffmpeg it uses, and apt installs them. URL import needs `curl` for
direct video links (a dependency) and `yt-dlp` for site pages and HLS
playlists, which is a **Recommends** so a normal `apt install` pulls it in;
systems installed with `--no-install-recommends` need `yt-dlp` installed
separately. The dialog's missing-helper message covers that case.

Remove the application files with `sudo apt remove lumina`. The package does
not own or remove user libraries, catalogs, configuration or caches. Lumina
stores configuration under `${XDG_CONFIG_HOME:-~/.config}/lumina`, thumbnails
under `${XDG_CACHE_HOME:-~/.cache}/lumina`, and its optional sample library
under `${XDG_DATA_HOME:-~/.local/share}/lumina/Sample Library`. These paths
remain after package removal. This behavior has been checked in an isolated
package install; broader distribution and upgrade testing remains open.

This package is unsigned and must not be distributed as a supported release.

## Flatpak (draft)

`packaging/flatpak/io.github.raysca.Lumina.yml` is the Flathub build: the
`org.gnome.Platform//50` runtime and SDK (GStreamer, GTK/GLib, Rust SDK
extension), a minimal LGPL-only ffmpeg CLI built from the same source and flags
as the macOS bundle, and the app itself from a pinned git commit with offline
cargo sources.

It requests `--filesystem=home` and `--filesystem=removable-media` on purpose:
a library is a folder anywhere on the user's disks and the catalog stores paths
relative to it, so portal document paths are not a stable identity across
rescans or volume remaps. `--device=dri`, `--socket=pulseaudio` and Wayland
with an X11 fallback cover rendering, audio and windowing. Expect to justify
the filesystem access in the Flathub review.

URL import currently needs `yt-dlp`, which is not bundled inside the sandbox
and is not present on the host, so the app reports its normal missing-helper
message there. Bundling a pinned copy is required before calling URL import
supported in the Flatpak. HEIC/HEIF and RAW previews remain unavailable on
Linux.

The manifest is a draft: it parses (`flatpak-builder --show-manifest`) and its
cargo sources are generated, but it has not been built end to end yet. The
build checklist (x86_64 and aarch64, sandbox launch, library on a removable
drive, video audio, deletion, `ffmpeg -version`) is in
[`packaging/flatpak/README.md`](../packaging/flatpak/README.md). Do not submit
it to Flathub until that passes and the release is tagged.

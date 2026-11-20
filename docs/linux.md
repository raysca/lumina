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

This package is still not a supported release. With the Linux release key
configured, CI also publishes it on the draft GitHub release as
`Lumina-vX.Y.Z-linux-amd64.deb` with `SHA256SUMS-linux.txt`, a detached
signature over those checksums and the public key. Without the key it stays an
internal workflow artifact. See [Release signing](#release-signing).

## Release signing

The `.deb` is not signed in-band: `debsigs` 0.01.19 (Ubuntu 24.04) implements
signing but not verification, so a signature inside the package is not
something a user can check with distribution tooling. The release signs the
checksums instead, which is what direct downloads can verify:

```sh
gpg --import lumina-linux-signing-key.asc
gpg --verify SHA256SUMS-linux.txt.asc SHA256SUMS-linux.txt
sha256sum --check SHA256SUMS-linux.txt
```

The key fingerprint is published with the release and is the release key, not
a personal one. A signed apt repository (signed `Release`/`InRelease`) is the
next step if Lumina is delivered through `apt`; Flathub signs the Flatpak build
on its side.

## Flatpak (draft)

`packaging/flatpak/io.github.raysca.Lumina.yml` is the Flathub build: the
`org.gnome.Platform//50` runtime and SDK (GStreamer, GTK/GLib, Rust SDK
extension), a minimal LGPL-only ffmpeg CLI built from the same source and flags
as the macOS bundle, a small `curl` built against the runtime's GnuTLS, the
pinned `yt-dlp` release, and the app itself from a pinned git commit with
offline cargo sources. All three helpers that URL import and previews rely on
are inside the sandbox; nothing comes from the host.

It requests `--filesystem=home`, `--filesystem=/media` and
`--filesystem=/run/media` on purpose: a library is a folder anywhere on the
user's disks and the catalog stores paths relative to it, so portal document
paths are not a stable identity across rescans or volume remaps. `--device=dri`,
`--socket=pulseaudio` and Wayland with an X11 fallback cover rendering, audio
and windowing. Expect to justify the filesystem access in the Flathub review.

HEIC/HEIF and RAW previews remain unavailable on Linux.

The manifest is a draft. An aarch64 build on a Linux host compiles and installs
the app with ffmpeg, `curl` and `yt-dlp` inside the sandbox, and it launches
under Xvfb (the AppStream screenshots were taken from that build). The CI job
builds x86_64 when the packaging inputs change. The remaining checks are the
manual ones on a Linux desktop (a library on a removable drive, video audio,
deletion, URL import), a release tag, and the Flathub review. They are listed
in [`packaging/flatpak/README.md`](../packaging/flatpak/README.md). Do not
submit it to Flathub until they pass and the release is tagged.

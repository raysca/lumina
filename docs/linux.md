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

Remove the application files with `sudo apt remove lumina`. The package does
not own or remove user libraries, catalogs, configuration or caches. Lumina
stores configuration under `${XDG_CONFIG_HOME:-~/.config}/lumina`, thumbnails
under `${XDG_CACHE_HOME:-~/.cache}/lumina`, and its optional sample library
under `${XDG_DATA_HOME:-~/.local/share}/lumina/Sample Library`. These paths
remain after package removal. This behavior has been checked in an isolated
package install; broader distribution and upgrade testing remains open.

This package is unsigned and must not be distributed as a supported release.

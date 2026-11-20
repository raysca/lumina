# Flatpak (draft)

This manifest is the Flathub build for Lumina. It is checked in so the
packaging decisions are reviewable, but it has not yet been built on a Linux
host: treat it as a draft until the checks below pass and CI grows a Flatpak
job.

## What it declares

- Runtime `org.gnome.Platform//50` with the Rust SDK extension. The GNOME
  runtime supplies GStreamer (playback) and GTK/GLib; the app links neither
  GTK directly but the SDK's headers match the build dependencies listed in
  `scripts/install_linux_deps.sh`.
- `--filesystem=home` and `--filesystem=removable-media`: the library is a
  folder the user picks and the catalog stores paths relative to it, so broad
  host access is the supported model. Portal document paths are not a stable
  identity across rescans and volume remaps. Expect to justify this in the
  Flathub review.
- `--device=dri` for Vulkan, `--socket=pulseaudio` for video audio,
  Wayland with an X11 fallback.
- A minimal LGPL-only ffmpeg CLI built from the same source and flags as the
  macOS bundle (`scripts/fetch_ffmpeg.sh`), installed beside the app.
- No network flag: Flatpak apps have network access by default, and Lumina
  only uses it for a URL the user pasted. There is no background traffic.

## Not bundled yet

- **URL import**: it needs `yt-dlp`, which is a Python program. Inside the
  sandbox there is no host `yt-dlp`, so the app reports its normal "helper is
  missing" message. Bundle a pinned `yt-dlp_linux` / `yt-dlp_linux_aarch64`
  before calling URL import supported in the Flatpak.
- **HEIC/HEIF and RAW previews**: still unsupported on Linux in this tree.
- **Screenshots** for the AppStream listing: the metainfo has none yet.

## Checks before submitting to Flathub

1. `flatpak-builder --user --install --force-clean build-dir packaging/flatpak/io.github.raysca.Lumina.yml`
   on x86_64 and aarch64, after installing the runtime, SDK and rust-stable
   extension.
2. Launch from the sandbox, open a library under `$HOME` and on a removable
   drive, play an H.264 video with audio, and check that thumbnails, posters
   and scrub previews decode (the bundled ffmpeg).
3. Add a library, quit, relaunch: the volume-remap and per-library cache
   behaviour must work with the sandbox's real paths.
4. Delete a library from Settings: its catalog and thumbnail folder go; the
   photos stay.
5. `flatpak run --command=sh io.github.raysca.Lumina` and confirm
   `/app/bin/ffmpeg -version` works and the license files are present.
6. Regenerate `cargo-sources.json` after any dependency change:

   ```sh
   python3 flatpak-cargo-generator.py ../../Cargo.lock -o cargo-sources.json
   ```

7. Bump the git `commit` to the release tag, add screenshots to the
   metainfo, and open the Flathub pull request.

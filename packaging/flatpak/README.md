# Flatpak (draft)

This manifest is the Flathub build for Lumina. It is checked in so the
packaging decisions are reviewable, and `.github/workflows/flatpak.yml` builds
it when the packaging inputs change. It is still a draft until the GUI checks
below pass on a Linux desktop and the release is tagged.

## What it declares

- Runtime `org.gnome.Platform//50` with the Rust SDK extension. The GNOME
  runtime supplies GStreamer (playback) and GTK/GLib; the app links neither
  GTK directly but the SDK's headers match the build dependencies listed in
  `scripts/install_linux_deps.sh`.
- `--filesystem=home`, `--filesystem=/media` and `--filesystem=/run/media`:
  the library is a folder the user picks and the catalog stores paths relative
  to it, so broad host access is the supported model. Portal document paths are
  not a stable identity across rescans and volume remaps. Expect to justify
  this in the Flathub review.
- `--device=dri` for Vulkan, `--socket=pulseaudio` for video audio,
  Wayland with an X11 fallback.
- A minimal LGPL-only ffmpeg CLI built from the same source and flags as the
  macOS bundle (`scripts/fetch_ffmpeg.sh`), installed beside the app.
- A small `curl` built against the runtime's GnuTLS, and the
  self-contained `yt-dlp_linux` / `yt-dlp_linux_aarch64` release the macOS
  bundle pins (public-domain yt-dlp, with its third-party licence file). Those
  three tools are what URL import runs; the host copies are not visible in the
  sandbox.
- Two AppStream screenshots under `resources/linux/screenshots/`, captured from
  the Flatpak under Xvfb, referenced by absolute `raw.githubusercontent.com`
  URLs so software centres and Flathub can fetch them.
- No network flag: Flatpak apps have network access by default, and Lumina
  only uses it for a URL the user pasted. There is no background traffic.

## Not bundled yet

- **HEIC/HEIF and RAW previews**: still unsupported on Linux in this tree.
- The manifest pins a main commit, not a release tag; Flathub wants a tag.

## Flathub listing checklist

- [x] App id, desktop id (`resources/linux/io.github.raysca.Lumina.desktop`),
      metainfo id and `provide`s binary agree.
- [x] Name, summary, description, categories, keywords and OARS rating in the
      metainfo; validated with `appstreamcli validate --no-net`.
- [x] Project and metadata licences (MIT) declared, with the font, icon,
      ffmpeg, curl and yt-dlp licences installed under
      `/app/share/licenses/io.github.raysca.Lumina`.
- [x] 512 px hicolor icon and AppStream launchable.
- [x] Screenshots in the metainfo, captured from the Flatpak under Xvfb at
      1440x900 and stored in `resources/linux/screenshots/`. The metainfo uses
      absolute `raw.githubusercontent.com` URLs, which resolve once the files
      are on `main`.
- [x] Sandbox permissions justified above.
- [ ] A release tag and a manifest `commit` pointing at it (currently a main
      commit).
- [x] A local aarch64 build (GNOME 50) compiled, installed and launched under
      Xvfb, with ffmpeg, curl and yt-dlp inside `/app/bin`.
- [ ] A green Flatpak CI run on x86_64 (the job builds every push that changes
      the packaging inputs).
- [ ] The manual checks above on a Linux desktop.
- [ ] An open Flathub pull request.

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
   `/app/bin/ffmpeg -hide_banner -version`, `/app/bin/yt-dlp --version` and
   `/app/bin/curl --version` work and the license files are present.
6. Paste a direct `.mp4` link and a YouTube link into Import Video from URL:
   both downloads must complete inside the sandbox.
7. Regenerate `cargo-sources.json` after any dependency change:

   ```sh
   python3 flatpak-cargo-generator.py ../../Cargo.lock -o cargo-sources.json
   ```

8. Bump the git `commit` to the release tag and open the Flathub pull
   request.

#!/usr/bin/env sh
# Assemble the GitHub Pages site into _site/ (or the directory given as $1).
# Fonts and brand art are copied from resources/, and app screenshots from
# docs/assets/screenshots/ (shared with the README), so the site never drifts from
# the app and the repository does not carry two copies of them.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-"$root/_site"}
rm -rf "$out"
mkdir -p "$out/assets/fonts" "$out/assets/brand" "$out/assets/screenshots"
cp -R "$root/site/." "$out/"
rm -f "$out/build.sh"
cp "$root/resources/fonts/Inter-Regular.otf" "$root/resources/fonts/Inter-Medium.otf" \
   "$root/resources/fonts/Inter-SemiBold.otf" "$root/resources/fonts/Inter-LICENSE.txt" \
   "$root/resources/fonts/InterTight-SemiBold.ttf" "$root/resources/fonts/InterTight-OFL.txt" \
   "$root/resources/fonts/JetBrainsMono-Regular.ttf" "$root/resources/fonts/JetBrainsMono-OFL.txt" \
   "$out/assets/fonts/"
cp "$root/resources/brand/lumina-icon.svg" "$root/resources/brand/lumina-mark.svg" "$out/assets/brand/"
cp "$root"/docs/assets/screenshots/*.webp "$out/assets/screenshots/"
echo "Site written to $out"

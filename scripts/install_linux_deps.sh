#!/usr/bin/env bash
# Installs the Ubuntu 24.04 packages Lumina needs to build, test, smoke-test
# and package on Linux. CI and the release workflow both run this, so the two
# lists cannot drift apart.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config dpkg-dev \
    libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
    libx11-dev libx11-xcb-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxcb-xkb-dev libxcb-randr0-dev libxcb-composite0-dev \
    libxcb-dri3-dev libxcb-present-dev libxcb-sync-dev libxcb-icccm4-dev libxcb-image0-dev \
    libxcb-keysyms1-dev libxcb-util-dev libxcb-cursor-dev libxcb-xinput-dev \
    libxcb-shm0-dev libxcb-xinerama0-dev libxi-dev libxcursor-dev \
    libxrandr-dev libxinerama-dev libxext-dev libxrender-dev \
    libasound2-dev libudev-dev libvulkan-dev mesa-vulkan-drivers \
    libfontconfig1-dev libfreetype6-dev libgtk-3-dev libglib2.0-dev \
    libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav ffmpeg \
    xvfb xauth x11-utils xdotool weston python3

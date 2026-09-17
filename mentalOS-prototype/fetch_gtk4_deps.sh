#!/usr/bin/env bash
# Fetch GTK4 dev libraries as .deb packages and extract them to a user-local prefix.
# This is a fallback for environments where sudo isn't available to apt install
# libgtk-4-dev system-wide.
#
# Usage: ./fetch_gtk4_deps.sh
#
# After running this, source ./setup_env.sh to activate the environment.

set -euo pipefail

PREFIX="${GTK4_PREFIX:-$HOME/gtk4-prefix}"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Fetching GTK4 dev packages into $TMPDIR ..."
cd "$TMPDIR"

apt-get download \
  libgtk-4-1 libgtk-4-common libgtk-4-dev \
  libgraphene-1.0-0 libgraphene-1.0-dev \
  libsoup-3.0-0 libsoup-3.0-common libsoup-3.0-dev \
  libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev libglib2.0-dev \
  libharfbuzz-dev libepoxy-dev libvulkan-dev libvulkan1 \
  wayland-protocols libwayland-dev libxkbcommon-dev libxkbcommon0 \
  libx11-dev libxcomposite-dev libxcursor-dev libxdamage-dev libxext-dev \
  libxfixes-dev libxi-dev libxinerama-dev libxrandr-dev \
  libegl-dev libegl1-mesa-dev libgl-dev libfontconfig-dev \
  libgirepository-2.0-dev \
  gir1.2-gtk-4.0 gir1.2-graphene-1.0 gir1.2-soup-3.0

echo "Extracting to $PREFIX ..."
mkdir -p "$PREFIX"
for deb in *.deb; do
  dpkg-deb -x "$deb" "$PREFIX/"
done

echo "Done. Now run: source ./setup_env.sh"

#!/usr/bin/env bash
# Source this file to set up the GTK4 environment for building mentalOS
# when GTK4 dev libraries aren't installed system-wide via sudo.
#
# Usage:
#   1. Run ./fetch_gtk4_deps.sh (one-time, downloads .deb packages)
#   2. source ./setup_env.sh
#
# This script is a no-op if GTK4 is already system-installed (pkg-config finds it).

if pkg-config --exists gtk4 2>/dev/null; then
  echo "GTK4 already available via pkg-config - no setup needed."
  return 0 2>/dev/null || exit 0
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" && pwd)"
GTK4_PREFIX="${GTK4_PREFIX:-$HOME/gtk4-prefix}"

if [ ! -d "$GTK4_PREFIX/usr" ]; then
  echo "ERROR: GTK4 prefix not found at $GTK4_PREFIX"
  echo "Run ./fetch_gtk4_deps.sh first, or install libgtk-4-dev via your package manager."
  return 1 2>/dev/null || exit 1
fi

export PKG_CONFIG_PATH="$GTK4_PREFIX/usr/lib/x86_64-linux-gnu/pkgconfig:$GTK4_PREFIX/usr/share/pkgconfig:${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="$GTK4_PREFIX/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"
export LIBRARY_PATH="$GTK4_PREFIX/usr/lib/x86_64-linux-gnu:${LIBRARY_PATH:-}"

echo "Environment configured for mentalOS build"
echo "  GTK4_PREFIX: $GTK4_PREFIX"
echo "  gtk4 version: $(pkg-config --modversion gtk4 2>/dev/null || echo 'not found')"

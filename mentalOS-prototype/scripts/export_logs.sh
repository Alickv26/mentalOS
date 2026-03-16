#!/usr/bin/env bash
set -euo pipefail

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="${1:-.}"
OUT_DIR="$(realpath -m "$OUT_DIR")"
OUT_FILE="${OUT_DIR}/mentalos-logs-${TIMESTAMP}.tar.gz"

LOG_DIR="${HOME}/.local/state/mentalOS/logs"
CFG_DIR="${HOME}/.config/mentalOS"

mkdir -p "$OUT_DIR"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

mkdir -p "${tmp_dir}/logs" "${tmp_dir}/config"

copy_if_exists() {
  local src="$1"
  local dest="$2"
  if [ -f "$src" ]; then
    cp "$src" "$dest"
  fi
}

copy_if_exists "${LOG_DIR}/mentalOS.log" "${tmp_dir}/logs/mentalOS.log"
copy_if_exists "${LOG_DIR}/mentalOS.log.1" "${tmp_dir}/logs/mentalOS.log.1"
copy_if_exists "${CFG_DIR}/config.toml" "${tmp_dir}/config/config.toml"
copy_if_exists "${CFG_DIR}/shortcuts.toml" "${tmp_dir}/config/shortcuts.toml"

cat > "${tmp_dir}/README.txt" <<'EOF'
mentalOS log bundle

Contents:
- logs/mentalOS.log(.1) if present
- config/config.toml if present
- config/shortcuts.toml if present

Review and redact secrets before sharing externally.
EOF

tar -C "$tmp_dir" -czf "$OUT_FILE" .
echo "Wrote log bundle: $OUT_FILE"

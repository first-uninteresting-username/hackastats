#!/usr/bin/env bash
set -euo pipefail

APPLET_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="${HOME}/.local/bin"
APP_ID="io.github.first_uninteresting_username.Hackastats"

RUST_VERSION="$(rustc --version | awk '{print $2}')"
if [ "$(printf '%s\n' '1.93.0' "$RUST_VERSION" | sort -V | head -n1)" != "1.93.0" ]; then
  printf 'HackaStats needs Rust 1.93 or newer; found %s.\n' "$RUST_VERSION" >&2
  printf 'Update the stable toolchain with: rustup update stable\n' >&2
  exit 1
fi

cd "$APPLET_DIR"
cargo build --release --locked

install -Dm0755 "target/release/cosmic-applet-hackastats" "$BIN_DIR/cosmic-applet-hackastats"
install -Dm0644 "target/xdgen/app.desktop" "$DATA_DIR/applications/$APP_ID.desktop"
install -Dm0644 "target/xdgen/app.metainfo.xml" "$DATA_DIR/metainfo/$APP_ID.metainfo.xml"
install -Dm0644 "resources/icon.svg" "$DATA_DIR/icons/hicolor/scalable/apps/$APP_ID.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$DATA_DIR/applications" >/dev/null 2>&1 || true
fi

printf '%s\n' \
  "HackaStats for COSMIC is installed." \
  "Add it to the top bar from: COSMIC Settings → Desktop → Panel → Applets → Add"

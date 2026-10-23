#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_ID="io.github.first_uninteresting_username.Hackastats"

rm -f \
  "$HOME/.local/bin/cosmic-applet-hackastats" \
  "$DATA_DIR/applications/$APP_ID.desktop" \
  "$DATA_DIR/metainfo/$APP_ID.metainfo.xml" \
  "$DATA_DIR/icons/hicolor/scalable/apps/$APP_ID.svg"

echo "HackaStats for COSMIC was removed."

#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-${HOME}/.local}"
APPDIR="${PREFIX}/lib/OnionHoster"
BINDIR="${PREFIX}/bin"
SHAREDIR="${PREFIX}/share"
DESKTOP_DIR="${SHAREDIR}/applications"
ICON_ROOT="${SHAREDIR}/icons/hicolor"

rm -f "${BINDIR}/onionhoster"
rm -rf "${APPDIR:?}"
rm -f "${DESKTOP_DIR}/OnionHoster.desktop"

for size in 32x32 64x64 128x128 256x256 512x512; do
  rm -f "${ICON_ROOT}/${size}/apps/onionhoster.png"
done

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${DESKTOP_DIR}" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index "${ICON_ROOT}" >/dev/null 2>&1 || true
fi

echo "Removed the portable OnionHoster installation from ${PREFIX}."
echo "Application data remains in ${XDG_DATA_HOME:-${HOME}/.local/share}/dev.uzair.onionhoster."

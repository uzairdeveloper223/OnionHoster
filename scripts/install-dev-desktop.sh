#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICON_ROOT="${XDG_DATA_HOME:-${HOME}/.local/share}/icons/hicolor"
APPS_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/applications"
DESKTOP_FILE="${APPS_DIR}/dev.uzair.onionhoster.desktop"

install -Dm644 "${ROOT}/src-tauri/icons/32x32.png" "${ICON_ROOT}/32x32/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/64x64.png" "${ICON_ROOT}/64x64/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/128x128.png" "${ICON_ROOT}/128x128/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/128x128@2x.png" "${ICON_ROOT}/256x256/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/icon.png" "${ICON_ROOT}/512x512/apps/onionhoster.png"

mkdir -p "${APPS_DIR}"
rm -f "${APPS_DIR}/onionhoster.desktop"
cat >"${DESKTOP_FILE}" <<EOF
[Desktop Entry]
Type=Application
Name=OnionHoster
GenericName=Tor service host
Comment=Run OnionHoster from source
Exec=bash -lc "cd '${ROOT}' && pnpm tauri dev"
Icon=onionhoster
Terminal=false
StartupWMClass=dev.uzair.onionhoster
Categories=Development;Network;
NoDisplay=true
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${APPS_DIR}" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index "${ICON_ROOT}" >/dev/null 2>&1 || true
fi

echo "Installed the development desktop entry at ${DESKTOP_FILE}."
echo "Restart OnionHoster to pick up the icon in the dock."

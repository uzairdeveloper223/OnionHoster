#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-${HOME}/.local}"
APPDIR="${PREFIX}/lib/OnionHoster"
BINDIR="${PREFIX}/bin"
SHAREDIR="${PREFIX}/share"
DESKTOP_DIR="${SHAREDIR}/applications"
ICON_ROOT="${SHAREDIR}/icons/hicolor"

if [[ ! -x "${ROOT}/bin/onionhoster" ]]; then
  echo "Missing ${ROOT}/bin/onionhoster. Run install.sh from the extracted tarball." >&2
  exit 1
fi

if [[ ! -d "${ROOT}/lib/OnionHoster/resources/tor" ]]; then
  echo "Missing the portable runtime under ${ROOT}/lib/OnionHoster/resources." >&2
  exit 1
fi

mkdir -p "${BINDIR}" "${APPDIR}" "${DESKTOP_DIR}" "${ICON_ROOT}"

install -m 755 "${ROOT}/bin/onionhoster" "${BINDIR}/onionhoster"

rm -rf "${APPDIR:?}/resources"
mkdir -p "${APPDIR}/resources"
cp -a "${ROOT}/lib/OnionHoster/resources/." "${APPDIR}/resources/"

if [[ -d "${ROOT}/share/icons/hicolor" ]]; then
  cp -a "${ROOT}/share/icons/hicolor/." "${ICON_ROOT}/"
fi

cat >"${DESKTOP_DIR}/OnionHoster.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=OnionHoster
GenericName=Tor service host
Comment=Host local sites and TCP services on Tor.
Exec="${BINDIR}/onionhoster"
Icon=onionhoster
Terminal=false
Categories=Network;Utility;
StartupWMClass=dev.uzair.onionhoster
EOF
chmod 644 "${DESKTOP_DIR}/OnionHoster.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${DESKTOP_DIR}" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index "${ICON_ROOT}" >/dev/null 2>&1 || true
fi

echo "Installed OnionHoster to ${APPDIR}."
echo "Binary: ${BINDIR}/onionhoster"
echo "Desktop entry: ${DESKTOP_DIR}/OnionHoster.desktop"
echo "Launch OnionHoster from the app grid or run ${BINDIR}/onionhoster."

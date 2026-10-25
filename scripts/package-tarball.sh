#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${ONIONHOSTER_VERSION:-$(node -p "require('${ROOT}/package.json').version")}"
ARCH="${ONIONHOSTER_ARCH:-x86_64}"
NAME="OnionHoster_${VERSION}_linux_${ARCH}"

BIN="${ROOT}/src-tauri/target/release/onionhoster"
TOR_DIR="${ROOT}/src-tauri/resources/tor"
CLOUDFLARED_DIR="${ROOT}/src-tauri/resources/cloudflared"
OUT_DIR="${ROOT}/src-tauri/target/release/bundle/tarball"

if [[ ! -x "${BIN}" ]]; then
  echo "Missing release binary: ${BIN}" >&2
  echo "Run: pnpm tauri build --bundles deb,appimage" >&2
  exit 1
fi

if [[ ! -x "${TOR_DIR}/tor" || ! -x "${TOR_DIR}/lyrebird" ]]; then
  echo "Missing Tor runtime in ${TOR_DIR}" >&2
  echo "Run: pnpm fetch:tor" >&2
  exit 1
fi

if [[ ! -x "${CLOUDFLARED_DIR}/cloudflared" ]]; then
  echo "Missing cloudflared in ${CLOUDFLARED_DIR}" >&2
  echo "Run: pnpm fetch:cloudflared" >&2
  exit 1
fi

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/onionhoster-tarball.XXXXXX")"
trap 'rm -rf "${STAGE}"' EXIT

DEST="${STAGE}/${NAME}"

mkdir -p \
  "${DEST}/bin" \
  "${DEST}/lib/OnionHoster/resources" \
  "${DEST}/share/icons/hicolor"

install -m 755 "${BIN}" "${DEST}/bin/onionhoster"
cp -a "${TOR_DIR}" "${DEST}/lib/OnionHoster/resources/tor"
cp -a "${CLOUDFLARED_DIR}" "${DEST}/lib/OnionHoster/resources/cloudflared"

install -Dm644 "${ROOT}/src-tauri/icons/32x32.png" \
  "${DEST}/share/icons/hicolor/32x32/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/64x64.png" \
  "${DEST}/share/icons/hicolor/64x64/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/128x128.png" \
  "${DEST}/share/icons/hicolor/128x128/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/128x128@2x.png" \
  "${DEST}/share/icons/hicolor/256x256/apps/onionhoster.png"
install -Dm644 "${ROOT}/src-tauri/icons/icon.png" \
  "${DEST}/share/icons/hicolor/512x512/apps/onionhoster.png"

install -m 755 "${ROOT}/scripts/tarball-install.sh" "${DEST}/install.sh"
install -m 755 "${ROOT}/scripts/tarball-uninstall.sh" "${DEST}/uninstall.sh"
install -m 644 "${ROOT}/README.md" "${DEST}/README.md"
install -m 644 "${ROOT}/LICENSE" "${DEST}/LICENSE"
install -m 644 "${ROOT}/THIRD_PARTY_NOTICES.md" "${DEST}/THIRD_PARTY_NOTICES.md"

cat >"${DEST}/README-PORTABLE.txt" <<EOF
OnionHoster ${VERSION} - portable Linux ${ARCH} build

Run without installing:
  ./bin/onionhoster

Install for the current user:
  ./install.sh

After installing, launch OnionHoster from the app grid or run:
  ~/.local/bin/onionhoster

Uninstall:
  ./uninstall.sh

Keep bin/ and lib/ together when running the tarball in place. The application
loads Tor and cloudflared from lib/OnionHoster/resources.

System requirements:
  WebKitGTK 4.1, GTK 3, librsvg, ca-certificates, and xdg-utils.

On Ubuntu 22.04 or newer:
  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 librsvg2-2 ca-certificates xdg-utils

On Fedora:
  sudo dnf install webkit2gtk4.1 gtk3 librsvg2 ca-certificates xdg-utils

On Arch:
  sudo pacman -S webkit2gtk-4.1 gtk3 librsvg ca-certificates xdg-utils

License: GPL-3.0-or-later. See LICENSE.
EOF

rm -rf "${OUT_DIR}"
mkdir -p "${OUT_DIR}"
tar \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  -C "${STAGE}" \
  -czf "${OUT_DIR}/${NAME}.tar.gz" \
  "${NAME}"

echo "Created ${OUT_DIR}/${NAME}.tar.gz"
sha256sum "${OUT_DIR}/${NAME}.tar.gz"

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/src-tauri/resources/tor"
TOR_VERSION="${TOR_VERSION:-15.0.26}"
BASE_URL="${TOR_BASE_URL:-https://dist.torproject.org/torbrowser/${TOR_VERSION}}"

case "$(uname -m)" in
  x86_64) PLATFORM="linux-x86_64" ;;
  aarch64 | arm64)
    echo "The Linux arm64 Tor Expert Bundle is not configured yet." >&2
    exit 1
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

BUNDLE="tor-expert-bundle-${PLATFORM}-${TOR_VERSION}.tar.gz"

if [ "${TOR_VERSION}" = "15.0.26" ] && [ "${PLATFORM}" = "linux-x86_64" ]; then
  TOR_SHA256="${TOR_SHA256:-19cf18726eecb362e0f21d27afa9ea9efe5da928bfdfbb1ceb5fe54519e87f4a}"
else
  : "${TOR_SHA256:?Set TOR_SHA256 for this Tor Expert Bundle version}"
fi

if [ "${FORCE:-0}" != "1" ] && [ -x "${DEST}/tor" ] && [ -x "${DEST}/lyrebird" ] &&
  [ -f "${DEST}/VERSION" ] && head -n 1 "${DEST}/VERSION" | grep -qx "tor ${TOR_VERSION}"; then
  echo "Tor ${TOR_VERSION} is already present in ${DEST}."
  exit 0
fi

TMP="$(mktemp -d)"
GNUPGHOME="${TMP}/gnupg"
export GNUPGHOME
mkdir -p "${GNUPGHOME}"
chmod 700 "${GNUPGHOME}"
trap 'rm -rf "${TMP}"' EXIT

KEY="${ROOT}/scripts/keys/tor-browser-signing-key.asc"
KEY_FPR="EF6E286DDA85EA2A4BA7DE684E2C6E8793298290"

if ! command -v gpg >/dev/null 2>&1; then
  echo "gpg is required to verify the Tor Expert Bundle signature." >&2
  exit 1
fi

gpg --batch --import "${KEY}" >/dev/null 2>&1
IMPORTED_FPR="$(gpg --batch --with-colons --import-options show-only --import "${KEY}" |
  awk -F: '/^fpr/ { print $10; exit }')"

if [ "${IMPORTED_FPR}" != "${KEY_FPR}" ]; then
  echo "Unexpected Tor signing key: ${IMPORTED_FPR}" >&2
  exit 1
fi

echo "Downloading Tor ${TOR_VERSION} for ${PLATFORM}..."
curl --fail --location --retry 3 --connect-timeout 20 \
  --output "${TMP}/${BUNDLE}" \
  "${BASE_URL}/${BUNDLE}"
curl --fail --location --retry 3 --connect-timeout 20 \
  --output "${TMP}/${BUNDLE}.asc" \
  "${BASE_URL}/${BUNDLE}.asc"

echo "Verifying checksum and signature..."
(
  cd "${TMP}"
  printf '%s  %s\n' "${TOR_SHA256}" "${BUNDLE}" | sha256sum --check --status
)
gpg --batch --verify "${TMP}/${BUNDLE}.asc" "${TMP}/${BUNDLE}" >/dev/null 2>&1

mkdir -p "${TMP}/extract"
tar -xzf "${TMP}/${BUNDLE}" -C "${TMP}/extract"

rm -rf "${DEST}"
mkdir -p "${DEST}"

install -m 755 "${TMP}/extract/tor/tor" "${DEST}/tor"
install -m 755 "${TMP}/extract/tor/pluggable_transports/lyrebird" "${DEST}/lyrebird"
find "${TMP}/extract/tor" -maxdepth 1 -name 'lib*.so*' -exec install -m 644 {} "${DEST}/" \;

for file in geoip geoip6 torrc-defaults; do
  if [ -f "${TMP}/extract/data/${file}" ]; then
    install -m 644 "${TMP}/extract/data/${file}" "${DEST}/${file}"
  fi
done

if [ -f "${TMP}/extract/tor/pluggable_transports/pt_config.json" ]; then
  install -m 644 "${TMP}/extract/tor/pluggable_transports/pt_config.json" \
    "${DEST}/pt_config.json"
fi

cat >"${DEST}/VERSION" <<EOF
tor ${TOR_VERSION}
source ${BASE_URL}/${BUNDLE}
sha256 ${TOR_SHA256}
EOF

echo "Installed Tor ${TOR_VERSION} in ${DEST}."
LD_LIBRARY_PATH="${DEST}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}" "${DEST}/tor" --version | head -n 1
"${DEST}/lyrebird" --version 2>&1 | head -n 1

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/src-tauri/resources/cloudflared"
VERSION="${CLOUDFLARED_VERSION:-2026.10.3}"

case "$(uname -m)" in
  x86_64) PLATFORM="linux-amd64" ;;
  aarch64 | arm64)
    echo "The cloudflared arm64 bundle is not configured yet." >&2
    exit 1
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

if [ "${VERSION}" = "2026.10.3" ] && [ "${PLATFORM}" = "linux-amd64" ]; then
  CLOUDFLARED_SHA256="${CLOUDFLARED_SHA256:-053b80b0a23031a038fdbf3c073f59f82b8f1f152aa67971d7383ffafc0dbab0}"
else
  : "${CLOUDFLARED_SHA256:?Set CLOUDFLARED_SHA256 for this cloudflared version}"
fi

if [ "${FORCE:-0}" != "1" ] && [ -x "${DEST}/cloudflared" ] &&
  [ -f "${DEST}/VERSION" ] && head -n 1 "${DEST}/VERSION" | grep -qx "cloudflared ${VERSION}"; then
  echo "cloudflared ${VERSION} is already present in ${DEST}."
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

URL="https://github.com/cloudflare/cloudflared/releases/download/${VERSION}/cloudflared-${PLATFORM}"

echo "Downloading cloudflared ${VERSION} for ${PLATFORM}..."
curl --fail --location --retry 3 --connect-timeout 20 \
  --output "${TMP}/cloudflared" \
  "${URL}"

echo "${CLOUDFLARED_SHA256}  ${TMP}/cloudflared" | sha256sum --check --status ||
  {
    echo "cloudflared SHA-256 verification failed." >&2
    exit 1
  }

install -Dm755 "${TMP}/cloudflared" "${DEST}/cloudflared"
printf 'cloudflared %s\n' "${VERSION}" > "${DEST}/VERSION"
echo "Installed cloudflared ${VERSION} in ${DEST}."

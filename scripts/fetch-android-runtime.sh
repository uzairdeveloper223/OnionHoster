#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARCH="${1:-aarch64}"
TOR_VERSION="16.0"
CLOUDFLARED_VERSION="2026.10.3"
SOURCE_SHA256="68d298c522aef45fe8412e0ccf8f21ff12a6ad768aa69cbc23e10eac688ac8c8"
case "${ARCH}" in
  aarch64)
    ABI="arm64-v8a"
    GOARCH="arm64"
    CLANG="aarch64-linux-android24-clang"
    TOR_SHA256="bf8f4c63652fd7daa62288713c7fa512f244a5b972808f6251450e5fdfca9120"
    ;;
  x86_64)
    ABI="x86_64"
    GOARCH="amd64"
    CLANG="x86_64-linux-android24-clang"
    TOR_SHA256="ab3152ba164bcd49e50175086c62e40ffa2bf9002a4f6473f77111bd88b0462b"
    ;;
  *) echo "Supported Android targets: aarch64, x86_64." >&2; exit 1 ;;
esac

: "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-${HOME}/Android/Sdk}}"
: "${NDK_HOME:=${ANDROID_HOME}/ndk/28.2.13676358}"
CC="${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/${CLANG}"
if [ ! -x "${CC}" ]; then
  echo "Install Android NDK 28.2.13676358 or set NDK_HOME to your NDK directory." >&2
  exit 1
fi
LIBS="${ROOT}/src-tauri/gen/android/app/src/main/jniLibs/${ABI}"
ASSETS="${ROOT}/src-tauri/resources/android"
STAMP="${ASSETS}/${ABI}.version"
VERSION="tor ${TOR_VERSION}; cloudflared ${CLOUDFLARED_VERSION}; ndk 28.2.13676358"
if [ "${FORCE:-0}" != "1" ] && [ -f "${STAMP}" ] &&
  [ "$(cat "${STAMP}")" = "${VERSION}" ] &&
  [ -f "${LIBS}/libTor.so" ] && [ -f "${LIBS}/liblyrebird.so" ] &&
  [ -f "${LIBS}/libcloudflared.so" ]; then
  cp "${ROOT}/LICENSE" "${ROOT}/THIRD_PARTY_NOTICES.md" "${ASSETS}/notices/"
  echo "Android runtime for ${ABI} is already present."
  exit 0
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/onionhoster-android.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT
BUNDLE="tor-expert-bundle-android-${ARCH}-${TOR_VERSION}.tar.gz"
BASE="https://dist.torproject.org/torbrowser/${TOR_VERSION}"
GNUPGHOME="${TMP}/gnupg"
export GNUPGHOME
mkdir -m 700 "${GNUPGHOME}"
KEY="${ROOT}/scripts/keys/tor-browser-signing-key.asc"
FINGERPRINT="$(gpg --batch --with-colons --import-options show-only --import "${KEY}" |
  awk -F: '/^fpr/ { print $10; exit }')"
test "${FINGERPRINT}" = "EF6E286DDA85EA2A4BA7DE684E2C6E8793298290"
gpg --batch --import "${KEY}" >/dev/null 2>&1

echo "Downloading Tor ${TOR_VERSION} for Android ${ABI}..."
curl -fSL --retry 3 --connect-timeout 20 "${BASE}/${BUNDLE}" -o "${TMP}/${BUNDLE}"
curl -fSL --retry 3 --connect-timeout 20 "${BASE}/${BUNDLE}.asc" -o "${TMP}/${BUNDLE}.asc"
echo "${TOR_SHA256}  ${TMP}/${BUNDLE}" | sha256sum --check --status
gpg --batch --verify "${TMP}/${BUNDLE}.asc" "${TMP}/${BUNDLE}" >/dev/null 2>&1
tar -xzf "${TMP}/${BUNDLE}" -C "${TMP}"

echo "Building cloudflared ${CLOUDFLARED_VERSION} for Android ${ABI}..."
curl -fSL --retry 3 --connect-timeout 20 \
  "https://codeload.github.com/cloudflare/cloudflared/tar.gz/refs/tags/${CLOUDFLARED_VERSION}" \
  -o "${TMP}/cloudflared.tar.gz"
echo "${SOURCE_SHA256}  ${TMP}/cloudflared.tar.gz" | sha256sum --check --status
tar -xzf "${TMP}/cloudflared.tar.gz" -C "${TMP}"
(
  cd "${TMP}/cloudflared-${CLOUDFLARED_VERSION}"
  # cgo uses Android's resolver and certificate store. netgo assumes /etc/resolv.conf.
  GOOS=android GOARCH="${GOARCH}" CGO_ENABLED=1 CC="${CC}" GOTOOLCHAIN=auto \
    go build -mod=readonly -trimpath -buildmode=pie \
      -ldflags "-s -w -X main.Version=${CLOUDFLARED_VERSION} -X main.BuildTime=source" \
      -o "${TMP}/libcloudflared.so" ./cmd/cloudflared
)

mkdir -p "${LIBS}" "${ASSETS}/runtime/tor" "${ASSETS}/runtime/cloudflared" "${ASSETS}/notices/tor"
install -m 755 "${TMP}/tor/libTor.so" "${LIBS}/libTor.so"
install -m 755 "${TMP}/tor/pluggable_transports/lyrebird" "${LIBS}/liblyrebird.so"
install -m 755 "${TMP}/libcloudflared.so" "${LIBS}/libcloudflared.so"
install -m 644 "${TMP}/data/geoip" "${TMP}/data/geoip6" "${ASSETS}/runtime/tor/"
cp "${TMP}/docs/"*.txt "${ASSETS}/notices/tor/"
cp "${TMP}/cloudflared-${CLOUDFLARED_VERSION}/LICENSE" "${ASSETS}/notices/cloudflared.txt"
cp "${ROOT}/LICENSE" "${ROOT}/THIRD_PARTY_NOTICES.md" "${ASSETS}/notices/"
printf 'tor %s\n' "${TOR_VERSION}" > "${ASSETS}/runtime/tor/VERSION"
printf 'cloudflared %s\n' "${CLOUDFLARED_VERSION}" > "${ASSETS}/runtime/cloudflared/VERSION"
printf '%s\n' "${VERSION}" > "${STAMP}"
echo "Installed Android runtime for ${ABI}."

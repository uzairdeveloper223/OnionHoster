#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-${HOME}/Android/Sdk}}"
: "${ANDROID_KEYSTORE_PATH:?Set ANDROID_KEYSTORE_PATH to your release keystore}"
: "${ANDROID_KEY_ALIAS:?Set ANDROID_KEY_ALIAS}"
: "${ANDROID_KEYSTORE_PASSWORD:?Set ANDROID_KEYSTORE_PASSWORD}"
: "${ANDROID_KEY_PASSWORD:?Set ANDROID_KEY_PASSWORD}"
TOOLS="${ANDROID_HOME}/build-tools/${ANDROID_BUILD_TOOLS_VERSION:-36.0.0}"
VERSION="$(node -p "require('${ROOT}/package.json').version")"
DEST="${ROOT}/src-tauri/target/release/bundle/android"
mkdir -p "${DEST}"
mapfile -t APKS < <(find "${ROOT}/src-tauri/gen/android/app/build/outputs/apk" -name '*-release-unsigned.apk' -type f)
if [ "${#APKS[@]}" != "1" ]; then
  echo "Expected one unsigned release APK, found ${#APKS[@]}. Build with --target aarch64 --apk." >&2
  exit 1
fi
APK="${DEST}/OnionHoster_${VERSION}_android_arm64.apk"
"${TOOLS}/zipalign" -f -P 16 4 "${APKS[0]}" "${DEST}/aligned.apk"
"${TOOLS}/apksigner" sign --ks "${ANDROID_KEYSTORE_PATH}" --ks-key-alias "${ANDROID_KEY_ALIAS}" \
  --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEY_PASSWORD \
  --out "${APK}" "${DEST}/aligned.apk"
rm "${DEST}/aligned.apk"
"${TOOLS}/apksigner" verify --verbose --print-certs "${APK}"
"${TOOLS}/zipalign" -c -P 16 4 "${APK}"
echo "Signed ${APK}."

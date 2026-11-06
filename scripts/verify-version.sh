#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(node -p "require('${ROOT}/package.json').version")"
TAURI="$(node -p "require('${ROOT}/src-tauri/tauri.conf.json').version")"
CARGO="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${ROOT}/src-tauri/Cargo.toml" | head -n 1)"
FRONTEND="$(sed -n 's/^export const APP_VERSION = "\([^"]*\)";/\1/p' "${ROOT}/src/lib/app.ts")"
test "${VERSION}" = "${TAURI}"
test "${VERSION}" = "${CARGO}"
test "${VERSION}" = "${FRONTEND}"
if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
  test "${VERSION}" = "${GITHUB_REF_NAME#v}"
fi
echo "OnionHoster ${VERSION} versions match."

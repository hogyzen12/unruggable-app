#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-unruggable}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
RELEASE_DIR="target/${TARGET_TRIPLE}/release"
BIN_PATH="${RELEASE_DIR}/${APP_NAME}"
OUT_NAME="${OUT_NAME:-unruggable-linux.zip}"
OUT_PATH="${RELEASE_DIR}/${OUT_NAME}"
SHA_PATH="${OUT_PATH}.sha256"
STAGE_DIR="${RELEASE_DIR}/${APP_NAME}-linux-package"

if [ ! -f "${BIN_PATH}" ]; then
  echo "❌ Missing Linux binary: ${BIN_PATH}"
  echo "   Build first with:"
  echo "   cross build --target ${TARGET_TRIPLE} --release --no-default-features --features desktop"
  exit 1
fi

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

cp "${BIN_PATH}" "${STAGE_DIR}/"
chmod +x "${STAGE_DIR}/${APP_NAME}"
cp -R assets "${STAGE_DIR}/assets"

revision="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
dirty="clean"
if ! git diff --quiet --ignore-submodules HEAD 2>/dev/null; then
  dirty="dirty"
fi
{
  echo "Unruggable app $(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
  echo "Target: ${TARGET_TRIPLE}"
  echo "Revision: ${revision} (${dirty})"
  echo "Packaged UTC: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
} > "${STAGE_DIR}/BUILD_INFO.txt"

rm -f "${OUT_PATH}" "${SHA_PATH}"
(
  cd "${STAGE_DIR}"
  zip -qry "../${OUT_NAME}" .
)

shasum -a 256 "${OUT_PATH}" | tee "${SHA_PATH}"

echo "✅ Linux release package ready:"
echo "   ${OUT_PATH}"
echo "   ${SHA_PATH}"
echo "   Targeted at Ubuntu 24.04-class systems with required WebKitGTK libs installed."

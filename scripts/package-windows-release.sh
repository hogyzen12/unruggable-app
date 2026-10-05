#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-unruggable}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-pc-windows-msvc}"
RELEASE_DIR="target/${TARGET_TRIPLE}/release"
EXE_PATH="${RELEASE_DIR}/${APP_NAME}.exe"
OUT_NAME="${OUT_NAME:-unruggable-app-windows.zip}"
OUT_PATH="${RELEASE_DIR}/${OUT_NAME}"
SHA_PATH="${OUT_PATH}.sha256"
STAGE_DIR="${RELEASE_DIR}/${APP_NAME}-windows-package"
SUPPORT_DOC="docs/WINDOWS_USB_SUPPORT.md"

required_dlls=(
  "WebView2Loader.dll"
)

if [ ! -f "${EXE_PATH}" ]; then
  echo "❌ Missing Windows binary: ${EXE_PATH}"
  echo "   Build first with:"
  echo "   cargo xwin build --target ${TARGET_TRIPLE} --release --no-default-features --features desktop"
  exit 1
fi

if [ ! -f "${SUPPORT_DOC}" ]; then
  echo "❌ Missing Windows support guide: ${SUPPORT_DOC}"
  exit 1
fi

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

cp "${EXE_PATH}" "${STAGE_DIR}/"

for dll in "${required_dlls[@]}"; do
  if [ -f "${RELEASE_DIR}/${dll}" ]; then
    cp "${RELEASE_DIR}/${dll}" "${STAGE_DIR}/"
  elif [ -f "windows_dlls/${dll}" ]; then
    cp "windows_dlls/${dll}" "${STAGE_DIR}/"
  else
    echo "❌ Missing required Windows runtime DLL: ${dll}"
    exit 1
  fi
done

cp -R assets "${STAGE_DIR}/assets"
find "${STAGE_DIR}" -name .DS_Store -delete
cp "${SUPPORT_DOC}" "${STAGE_DIR}/WINDOWS_USB_SUPPORT.md"

revision="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
dirty="clean"
if ! git diff --quiet --ignore-submodules HEAD 2>/dev/null; then
  dirty="dirty"
fi
{
  echo "Unruggable app $(sed -n 's/^version = \"\(.*\)\"/\1/p' Cargo.toml | head -1)"
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

echo "✅ Windows release package ready:"
echo "   ${OUT_PATH}"
echo "   ${SHA_PATH}"

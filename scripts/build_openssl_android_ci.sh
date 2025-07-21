
#!/usr/bin/env bash
set -euo pipefail

# ──────────────── Config ────────────────

OPENSSL_VERSION="3.0.16"
OPENSSL_TARBALL="openssl-${OPENSSL_VERSION}.tar.gz"
OPENSSL_URL="https://www.openssl.org/source/${OPENSSL_TARBALL}"
BUILD_ROOT="$HOME/openssl-${OPENSSL_VERSION}"
INSTALL_DIR="${BUILD_ROOT}/android-build"

# This script is for CI, where ANDROID_NDK_HOME is always provided.
if [ -z "${ANDROID_NDK_HOME-}" ]; then
  echo "Error: ANDROID_NDK_HOME is not set. This script is intended for CI use."
  exit 1
fi
NDK_HOME="${ANDROID_NDK_HOME}"

# Minimum Android API level
API=24

# Set up toolchain paths
HOST_TAG="linux-x86_64"
TOOLCHAIN="${NDK_HOME}/toolchains/llvm/prebuilt/${HOST_TAG}"
export PATH="${TOOLCHAIN}/bin:${PATH}"
export ANDROID_NDK_ROOT="${NDK_HOME}"

# ─────────── Download & unpack OpenSSL ───────────

mkdir -p "${BUILD_ROOT}"
cd "${BUILD_ROOT}"

if [ ! -f "${OPENSSL_TARBALL}" ]; then
  echo "Downloading OpenSSL ${OPENSSL_VERSION}..."
  wget "${OPENSSL_URL}"
fi

if [ -d "openssl-${OPENSSL_VERSION}" ]; then
  echo "Removing previous source directory..."
  rm -rf "openssl-${OPENSSL_VERSION}"
fi

echo "Extracting source..."
tar xzf "${OPENSSL_TARBALL}"
cd "openssl-${OPENSSL_VERSION}"

# ───────── Configure, build & install ─────────

echo "Configuring for android-arm64 (API ${API})..."
./Configure android-arm64 \
  -D__ANDROID_API__=${API} \
  --prefix="${INSTALL_DIR}" \
  --openssldir="${INSTALL_DIR}" \
  --libdir=lib

echo "Building (make -j)…"
make -j"$(nproc)"

echo "Installing to ${INSTALL_DIR}…"
make install_sw

echo "✅ OpenSSL ${OPENSSL_VERSION} for aarch64-linux-android built and installed to ${INSTALL_DIR}"


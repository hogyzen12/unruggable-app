
#!/usr/bin/env bash
set -euo pipefail

# ──────────────── Config ────────────────

OPENSSL_VERSION="3.0.16"
OPENSSL_TARBALL="openssl-${OPENSSL_VERSION}.tar.gz"
OPENSSL_URL="https://www.openssl.org/source/${OPENSSL_TARBALL}"
BUILD_ROOT="$HOME/openssl-${OPENSSL_VERSION}"
INSTALL_DIR="${BUILD_ROOT}/android-build"

# Pick your NDK installation here
# (must match what you have under $HOME/Library/Android/sdk/ndk/)
NDK_HOME="${HOME}/Library/Android/sdk/ndk/29.0.13599879"

# Minimum Android API level
API=24

# ───── Detect host toolchain directory ─────

HOST_TAG="darwin-x86_64"
# Newer NDKs may ship darwin-arm64 as well
if [ -d "${NDK_HOME}/toolchains/llvm/prebuilt/darwin-arm64" ]; then
  HOST_TAG="darwin-arm64"
fi
TOOLCHAIN="${NDK_HOME}/toolchains/llvm/prebuilt/${HOST_TAG}"
CLANG="${TOOLCHAIN}/bin/aarch64-linux-android${API}-clang"
AR_TOOL="${TOOLCHAIN}/bin/llvm-ar"
RANLIB_TOOL="${TOOLCHAIN}/bin/llvm-ranlib"

# ─────────────── Export env ────────────────

export ANDROID_NDK_ROOT="${NDK_HOME}"
export PATH="${TOOLCHAIN}/bin:${PATH}"

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

# ───────── Configure, build & install ─────────

cd "openssl-${OPENSSL_VERSION}"

echo "Configuring for android-arm64 (API ${API})..."
./Configure android-arm64 \
  -D__ANDROID_API__=${API} \
  --prefix="${INSTALL_DIR}" \
  --openssldir="${INSTALL_DIR}"

echo "Building (make -j)…"
make -j"$(sysctl -n hw.ncpu)"

echo "Installing to ${INSTALL_DIR}…"
make install_sw

echo "✅ OpenSSL ${OPENSSL_VERSION} built and installed to ${INSTALL_DIR}"

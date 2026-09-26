#!/usr/bin/env bash
# Build static FFmpeg libraries for videocanvas release CI (Linux/macOS).
# Any failure aborts the script (set -euo pipefail) so CI goes red.
#
# Env knobs (all optional):
#   FFMPEG_VERSION  FFmpeg release to build (default 7.1.5).
#   FFMPEG_PREFIX   Install prefix (default $GITHUB_WORKSPACE/ffmpeg-build,
#                   falls back to $PWD/ffmpeg-build outside GitHub Actions).
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-7.1.5}"
WORKSPACE="${GITHUB_WORKSPACE:-$PWD}"
FFMPEG_PREFIX="${FFMPEG_PREFIX:-$WORKSPACE/ffmpeg-build}"

if [ -f "$FFMPEG_PREFIX/lib/pkgconfig/libavformat.pc" ]; then
  echo "FFmpeg already built at $FFMPEG_PREFIX, skipping."
else
  echo "Building static FFmpeg $FFMPEG_VERSION into $FFMPEG_PREFIX ..."
  BUILD_DIR="$(mktemp -d)"
  trap 'rm -rf "$BUILD_DIR"' EXIT
  curl --fail --location --output "$BUILD_DIR/ffmpeg.tar.xz" \
    "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
  tar -xf "$BUILD_DIR/ffmpeg.tar.xz" -C "$BUILD_DIR"
  cd "$BUILD_DIR/ffmpeg-$FFMPEG_VERSION"
  ./configure \
    --prefix="$FFMPEG_PREFIX" \
    --disable-shared \
    --enable-static \
    --disable-programs \
    --disable-doc \
    --disable-debug \
    --pkg-config-flags="--static"
  make -j"$(nproc 2>/dev/null || sysctl -n hw.ncpu)"
  make install
fi

# Export link configuration for later workflow steps.
if [ -n "${GITHUB_ENV:-}" ]; then
  {
    echo "FFMPEG_DIR=$FFMPEG_PREFIX"
    echo "PKG_CONFIG_PATH=$FFMPEG_PREFIX/lib/pkgconfig"
    echo "PKG_CONFIG_ALL_STATIC=1"
  } >>"$GITHUB_ENV"
else
  echo "export FFMPEG_DIR=$FFMPEG_PREFIX"
  echo "export PKG_CONFIG_PATH=$FFMPEG_PREFIX/lib/pkgconfig"
  echo "export PKG_CONFIG_ALL_STATIC=1"
fi

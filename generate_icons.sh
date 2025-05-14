#!/usr/bin/env bash
# generate_icons.sh  (IM‑7‑friendly)

set -euo pipefail

SRC=${1:-icon.png}
OUT=${2:-icons}
SIZES=(16 24 32 64 128 256)

need() { command -v "$1" &>/dev/null || {
  echo "Error: '$1' is required. Install with brew (e.g. 'brew install $2')." >&2; exit 1; }; }

# ---------- prerequisites ----------
need iconutil iconutil        # Xcode CLT
need sips     sips            # macOS builtin
# pick ImageMagick command
if command -v magick &>/dev/null; then IM="magick"; else need convert imagemagick; IM="convert"; fi
# -----------------------------------

[[ -f $SRC ]] || { echo "Source image '$SRC' not found." >&2; exit 1; }
mkdir -p "$OUT"

echo "▶  Creating resized PNGs…"
for sz in "${SIZES[@]}"; do
  $IM "$SRC" -resize "${sz}x${sz}" \
      -gravity center -background none -extent "${sz}x${sz}" \
      -depth 8 PNG32:"$OUT/${sz}x${sz}.png"
done
cp "$OUT/256x256.png" "$OUT/128x128@2x.png"

echo "▶  Creating Windows .ico…"
$IM "$SRC" -define icon:auto-resize=16,24,32,48,64,128,256 \
     PNG32:"$OUT/icon.ico"

echo "▶  Creating macOS .icns…"
TMPDIR=$(mktemp -d)
ICONSET="$TMPDIR/app.iconset"; mkdir "$ICONSET"

cp "$OUT/16x16.png"   "$ICONSET/icon_16x16.png"
sips -z 32 32   "$SRC" --out "$ICONSET/icon_16x16@2x.png" &>/dev/null
cp "$OUT/32x32.png"   "$ICONSET/icon_32x32.png"
sips -z 64 64   "$SRC" --out "$ICONSET/icon_32x32@2x.png" &>/dev/null
cp "$OUT/128x128.png" "$ICONSET/icon_128x128.png"
cp "$OUT/256x256.png" "$ICONSET/icon_128x128@2x.png"
sips -z 256 256 "$SRC" --out "$ICONSET/icon_256x256.png"   &>/dev/null
sips -z 512 512 "$SRC" --out "$ICONSET/icon_256x256@2x.png" &>/dev/null
sips -z 512 512 "$SRC" --out "$ICONSET/icon_512x512.png"    &>/dev/null
sips -z 1024 1024 "$SRC" --out "$ICONSET/icon_512x512@2x.png" &>/dev/null

iconutil -c icns "$ICONSET" -o "$OUT/icon.icns"
rm -rf "$TMPDIR"

echo -e "\n✅  All clean — generated assets in '$OUT/'"

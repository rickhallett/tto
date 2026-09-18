#!/bin/sh
# Turn a square PNG into an .icns using only what ships with macOS.
#   scripts/make-icns.sh <in.png> <out.icns>
set -eu
src=$1
dst=$2
tmp=$(mktemp -d)
set_dir="$tmp/tto.iconset"
mkdir -p "$set_dir"
for size in 16 32 128 256 512; do
  double=$((size * 2))
  sips -z "$size" "$size" "$src" --out "$set_dir/icon_${size}x${size}.png" >/dev/null
  sips -z "$double" "$double" "$src" --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$dst"
rm -rf "$tmp"

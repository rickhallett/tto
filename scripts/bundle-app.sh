#!/bin/sh
# Wrap the nuke binary in a Turn Them Off.app bundle for people who don't do terminals.
#
#   scripts/bundle-app.sh <path-to-nuke-binary> <version> <output-dir>
#
# The bundle's executable IS the CLI binary; with no arguments and a
# Contents/MacOS parent it starts in menu bar mode. LSUIElement keeps it
# out of the Dock. Ad-hoc signed so Apple Silicon will run it at all.
set -eu

bin=$1
version=$2
out=$3
app="$out/Turn Them Off.app"
here=$(cd "$(dirname "$0")" && pwd)

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/tto"
chmod 755 "$app/Contents/MacOS/tto"

# Prefer a raster logo if one has been dropped in; otherwise rasterise the SVG.
if [ -f "$here/../assets/logo.png" ]; then
  logo="$here/../assets/logo.png"
else
  logo=$(mktemp -t tto-logo).png
  swift "$here/render-svg.swift" "$here/../assets/logo.svg" "$logo" 1024
fi
sh "$here/make-icns.sh" "$logo" "$app/Contents/Resources/tto.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>                 <string>Turn Them Off</string>
  <key>CFBundleDisplayName</key>          <string>Turn Them Off</string>
  <key>CFBundleIdentifier</key>           <string>dev.oceanheart.tto</string>
  <key>CFBundleVersion</key>              <string>$version</string>
  <key>CFBundleShortVersionString</key>   <string>$version</string>
  <key>CFBundlePackageType</key>          <string>APPL</string>
  <key>CFBundleExecutable</key>           <string>tto</string>
  <key>CFBundleIconFile</key>             <string>tto</string>
  <key>LSMinimumSystemVersion</key>       <string>12.0</string>
  <key>LSUIElement</key>                  <true/>
  <key>NSHumanReadableCopyright</key>     <string>MIT. Get some rest.</string>
</dict>
</plist>
PLIST

printf 'APPL????' > "$app/Contents/PkgInfo"
codesign --force --sign - --identifier dev.oceanheart.tto "$app"
echo "$app"

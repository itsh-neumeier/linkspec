#!/usr/bin/env bash
# Build LinkSpec.app (arm64) with embedded CLI, a universal CLI (Intel slice
# only prints the "not supported" message), and .dmg/.zip.
# Signing and notarization run only when MACOS_SIGN_IDENTITY is set
# (docs/MAINTAINER_TODO.md).
set -euo pipefail
VERSION="${VERSION:-$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys;print([p for p in json.load(sys.stdin)["packages"] if p["name"]=="linkspec-cli"][0]["version"])')}"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
cargo build --release --locked --target aarch64-apple-darwin -p linkspec-cli -p linkspec-app
cargo build --release --locked --target x86_64-apple-darwin -p linkspec-cli
OUT=dist; mkdir -p "$OUT"
lipo -create -output "$OUT/linkspec" \
  target/aarch64-apple-darwin/release/linkspec target/x86_64-apple-darwin/release/linkspec
APP="$OUT/LinkSpec.app"; rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/aarch64-apple-darwin/release/linkspec-app "$APP/Contents/MacOS/"
cp "$OUT/linkspec" "$APP/Contents/MacOS/linkspec"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
ICONSET="$OUT/LinkSpec.iconset"; rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  cp "packaging/icons/linkspec-$s.png" "$ICONSET/icon_${s}x${s}.png"
  d=$((s*2)); [ -f "packaging/icons/linkspec-$d.png" ] && cp "packaging/icons/linkspec-$d.png" "$ICONSET/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/LinkSpec.icns"
cp LICENSE THIRD_PARTY_LICENSES "$APP/Contents/Resources/"
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$APP/Contents/MacOS/linkspec"
  codesign --force --options runtime --timestamp --entitlements packaging/macos/entitlements.plist --sign "$MACOS_SIGN_IDENTITY" "$APP"
  codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$OUT/linkspec"
fi
(cd "$OUT" && ditto -c -k --keepParent LinkSpec.app "LinkSpec-$VERSION-macos-arm64.zip")
hdiutil create -volname LinkSpec -srcfolder "$APP" -ov -format UDZO "$OUT/LinkSpec-$VERSION.dmg"
tar -czf "$OUT/linkspec-$VERSION-macos-universal.tar.gz" -C "$OUT" linkspec
if [ -n "${APPLE_ID:-}" ]; then
  xcrun notarytool submit "$OUT/LinkSpec-$VERSION.dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
  xcrun stapler staple "$OUT/LinkSpec-$VERSION.dmg"
fi
(cd "$OUT" && shasum -a 256 ./*.dmg ./*.zip ./*.tar.gz > SHA256SUMS-macos)

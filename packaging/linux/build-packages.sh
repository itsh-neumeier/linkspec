#!/usr/bin/env bash
# Build Linux release artefacts: tarball, .deb, .rpm, AppImage.
# Needs: cargo-deb, cargo-generate-rpm, appimagetool (optional).
# Usage: packaging/linux/build-packages.sh [target-triple]
set -euo pipefail
TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys;print([p for p in json.load(sys.stdin)["packages"] if p["name"]=="linkspec-cli"][0]["version"])')"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
cargo build --release --locked --target "$TARGET" -p linkspec-cli -p linkspec-app
OUT=dist; mkdir -p "$OUT"
BIN="target/$TARGET/release"
ARCH="${TARGET%%-*}"
# Tarball
STAGE="$OUT/linkspec-$VERSION-linux-$ARCH"
rm -rf "$STAGE"; mkdir -p "$STAGE"
cp "$BIN/linkspec" "$BIN/linkspec-app" README.md LICENSE THIRD_PARTY_LICENSES packaging/linux/linkspec.desktop packaging/linux/60-linkspec.rules "$STAGE/"
cp packaging/icons/linkspec-256.png "$STAGE/linkspec.png"
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner -czf "$STAGE.tar.gz" -C "$OUT" "$(basename "$STAGE")"
# deb / rpm (metadata in app/Cargo.toml)
if command -v cargo-deb >/dev/null; then cargo deb -p linkspec-app --no-build --target "$TARGET" -o "$OUT/"; fi
if command -v cargo-generate-rpm >/dev/null; then cargo generate-rpm -p app --target "$TARGET" -o "$OUT/"; fi
# AppImage
if command -v appimagetool >/dev/null; then
  APPDIR="$OUT/LinkSpec.AppDir"; rm -rf "$APPDIR"; mkdir -p "$APPDIR/usr/bin"
  cp "$BIN/linkspec" "$BIN/linkspec-app" "$APPDIR/usr/bin/"
  cp packaging/linux/linkspec.desktop "$APPDIR/linkspec.desktop"
  cp packaging/icons/linkspec-256.png "$APPDIR/linkspec.png"
  printf '#!/bin/sh\nexec "$(dirname "$0")/usr/bin/linkspec-app" "$@"\n' > "$APPDIR/AppRun"; chmod +x "$APPDIR/AppRun"
  ARCH="$ARCH" appimagetool "$APPDIR" "$OUT/LinkSpec-$VERSION-$ARCH.AppImage"
fi
(cd "$OUT" && sha256sum ./*.tar.gz ./*.deb ./*.rpm ./*.AppImage 2>/dev/null > SHA256SUMS-linux-$ARCH || true)

#!/bin/sh
# Packages the built .app into a simple compressed DMG (no Finder styling, works headless).
set -e
APP="src-tauri/target/release/bundle/macos/Personal Workspace.app"
OUT="src-tauri/target/release/bundle/Personal Workspace_0.1.0_aarch64.dmg"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$OUT"
hdiutil create -volname "Personal Workspace" -srcfolder "$STAGE" -ov -format UDZO "$OUT"
rm -rf "$STAGE"
echo "$OUT"

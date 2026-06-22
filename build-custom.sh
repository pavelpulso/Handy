#!/usr/bin/env bash
# Build Handy (with Codex Dictation + Groq models) on x86_64 macOS,
# then strip the hardened runtime so the adhoc-signed build can open the
# microphone without CoreAudio/TCC hanging. Installs to /Applications.
set -euo pipefail

cd "$(dirname "$0")"

ORT_DYLIB="${ORT_DYLIB_PATH:-/usr/local/lib/libonnxruntime.dylib}"
APP_SRC="src-tauri/target/release/bundle/macos/Handy.app"
APP_DST="/Applications/Handy.app"

echo "==> Building (this recompiles whisper.cpp; takes a while)"
CMAKE_POLICY_VERSION_MINIMUM=3.5 ORT_DYLIB_PATH="$ORT_DYLIB" bun run tauri build

echo "==> Re-signing without hardened runtime (adhoc)"
codesign --remove-signature "$APP_SRC" 2>/dev/null || true
codesign --force --deep --sign - "$APP_SRC"
codesign -dv --verbose=2 "$APP_SRC" 2>&1 | grep -i flags

echo "==> Installing to $APP_DST"
osascript -e 'quit app "Handy"' 2>/dev/null || true
pkill -f "Handy.app" 2>/dev/null || true
sleep 1
rm -rf "$APP_DST"
cp -R "$APP_SRC" "$APP_DST"
xattr -dr com.apple.quarantine "$APP_DST" 2>/dev/null || true

echo "==> Done. Launch: open $APP_DST"

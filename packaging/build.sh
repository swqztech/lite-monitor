#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="$ROOT/packaging/stage"
DIST="$ROOT/dist"
VERSION="$(grep -E '^\s*version\s*=' "$ROOT/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"

(cd "$ROOT" && cargo build --release)
EXE_SRC="$ROOT/target/release/lite-monitor.exe"
[[ -f "$EXE_SRC" ]] || exit 1

PORTABLE_NAME="LiteMonitor-${VERSION}-win64-绿色版"
PORTABLE_DIR="$DIST/$PORTABLE_NAME"
rm -rf "$DIST"
mkdir -p "$PORTABLE_DIR"
cp -f "$EXE_SRC" "$PORTABLE_DIR/LiteMonitor.exe"
cp -f "$STAGE/启动.bat" "$STAGE/使用说明.txt" "$PORTABLE_DIR/"
(cd "$DIST" && tar -a -cf "${PORTABLE_NAME}.zip" "$(basename "$PORTABLE_DIR")")
echo "完成。输出目录: $DIST"

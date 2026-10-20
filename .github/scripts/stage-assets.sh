#!/usr/bin/env bash
# 把 Tauri 打出来的安装包重命名为带版本号的发行文件名，并收集到 dist/。
#
# 用法: stage-assets.sh <version> <target-triple>
# 产物示例:
#   LycorisWallet_0.1.0_x64-setup.exe
#   LycorisWallet_0.1.0_amd64.AppImage
#   LycorisWallet_0.1.0_arm64.dmg
set -euo pipefail

VERSION="${1:?需要版本号}"
TARGET="${2:?需要 target 三元组}"

# 本仓库是 Cargo workspace，cargo 的 target 目录在仓库根目录；
# 如果以后拆成独立 Tauri 工程，产物则落在 src-tauri/target。两种都兼容。
cd "$(dirname "$0")/../.."
OUT_DIR=""
for candidate in "target/${TARGET}/release" "src-tauri/target/${TARGET}/release"; do
  if [ -d "$candidate/bundle" ]; then
    OUT_DIR="$candidate"
    break
  fi
done
if [ -z "$OUT_DIR" ]; then
  echo "找不到打包目录: 已尝试 target/${TARGET}/release/bundle 与 src-tauri/target/${TARGET}/release/bundle" >&2
  exit 1
fi
BUNDLE="$OUT_DIR/bundle"
BIN_DIR="$OUT_DIR"
DIST="dist"

case "$TARGET" in
  x86_64-pc-windows-msvc)    ARCH="x64";   PLATFORM="windows" ;;
  x86_64-apple-darwin)       ARCH="x64";   PLATFORM="macos" ;;
  aarch64-apple-darwin)      ARCH="arm64"; PLATFORM="macos" ;;
  x86_64-unknown-linux-gnu)  ARCH="amd64"; PLATFORM="linux" ;;
  aarch64-unknown-linux-gnu) ARCH="arm64"; PLATFORM="linux" ;;
  *)
    echo "未知的 target: $TARGET" >&2
    exit 1
    ;;
esac

if [ ! -d "$BUNDLE" ]; then
  echo "找不到打包目录: $BUNDLE" >&2
  exit 1
fi

mkdir -p "$DIST"
NAME="LycorisWallet_${VERSION}"

shopt -s nullglob
for file in "$BUNDLE"/*/*; do
  [ -f "$file" ] || continue
  base="$(basename "$file")"
  case "$base" in
    *.exe)        out="${NAME}_${ARCH}-setup.exe" ;;
    *.msi)        out="${NAME}_${ARCH}.msi" ;;
    *.AppImage)   out="${NAME}_${ARCH}.AppImage" ;;
    *.deb)        out="${NAME}_${ARCH}.deb" ;;
    *.rpm)        out="${NAME}_${ARCH}.rpm" ;;
    *.dmg)        out="${NAME}_${ARCH}.dmg" ;;
    *.app.tar.gz) out="${NAME}_${ARCH}.app.tar.gz" ;;
    *)            continue ;;
  esac
  cp "$file" "$DIST/$out"
  echo "  $base  ->  $out"
done

# macOS 的 .app 是目录，打成 tar.gz 一并发布
for app in "$BUNDLE"/*/*.app; do
  [ -d "$app" ] || continue
  tar -czf "$DIST/${NAME}_${ARCH}.app.tar.gz" -C "$(dirname "$app")" "$(basename "$app")"
  echo "  $(basename "$app")  ->  ${NAME}_${ARCH}.app.tar.gz"
done

# 裸二进制，方便高级用户直接运行
if [ -f "$BIN_DIR/lycoris-wallet.exe" ]; then
  cp "$BIN_DIR/lycoris-wallet.exe" "$DIST/${NAME}_${PLATFORM}-${ARCH}.exe"
elif [ -f "$BIN_DIR/lycoris-wallet" ]; then
  cp "$BIN_DIR/lycoris-wallet" "$DIST/${NAME}_${PLATFORM}-${ARCH}"
fi

echo "---- dist/ ----"
ls -lh "$DIST"

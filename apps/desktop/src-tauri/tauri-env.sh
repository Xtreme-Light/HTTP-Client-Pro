#!/usr/bin/env bash
# Tauri Linux 开发环境变量 — 手动解压的系统库 sysroot。
# 用法: source apps/desktop/src-tauri/tauri-env.sh && pnpm tauri dev

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSROOT="/tmp/tauri-sysroot"

if [ ! -d "$SYSROOT/usr/lib" ]; then
    echo "⚠ 系统库 sysroot 不存在: $SYSROOT"
    echo "  请先运行: ./apps/desktop/src-tauri/setup-deps.sh"
    exit 1
fi

export PKG_CONFIG_PATH="$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig:$SYSROOT/usr/share/pkgconfig"
export C_INCLUDE_PATH="$SYSROOT/usr/include:$SYSROOT/usr/include/x86_64-linux-gnu:$SYSROOT/usr/lib/x86_64-linux-gnu/glib-2.0/include"
export LIBRARY_PATH="$SYSROOT/usr/lib/x86_64-linux-gnu"
export LD_LIBRARY_PATH="$SYSROOT/usr/lib/x86_64-linux-gnu"

# gio-2.0 的 .pc 文件将 selinux 和 mount 声明为 Requires.private，
# pkg-config --libs 不输出它们，但静态链接 libgio-2.0.a 时需要这些符号。
export RUSTFLAGS="-C link-arg=-lselinux -C link-arg=-lmount"

echo "✓ Tauri sysroot 已配置: $SYSROOT"

#!/usr/bin/env bash
# 下载并解压 Tauri v2 Linux 所需的系统开发库到 /tmp/tauri-sysroot。
# 无需 sudo — 使用 apt-get download + dpkg -x。
# 用法: bash apps/desktop/src-tauri/setup-deps.sh

set -euo pipefail

DEPS_DIR="/tmp/tauri-deps"
SYSROOT="/tmp/tauri-sysroot"

mkdir -p "$DEPS_DIR" "$SYSROOT"
cd "$DEPS_DIR"

echo "==> 下载开发包..."
# 递归获取所有 -dev 依赖
apt-cache depends --recurse --no-recommends --no-suggests --no-conflicts \
    --no-breaks --no-replaces --no-enhances --no-pre-depends \
    libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    librsvg2-dev libxkbcommon-dev libxdo-dev \
    2>/dev/null \
    | grep '^\w' \
    | grep -E '\-dev$|lib.*-dev' \
    | grep -vi 'doc\|dbg\|common\|tools\|aspell\|hunspell\|hyphen\|mythes\|fortran' \
    | sort -u > /tmp/tauri-pkgs.txt

xargs apt-get download < /tmp/tauri-pkgs.txt

echo "==> 下载运行时库..."
apt-get download \
    libwebkit2gtk-4.1-0 libjavascriptcoregtk-4.1-0 \
    libpango-1.0-0 libpangocairo-1.0-0 \
    libgdk-pixbuf-2.0-0 libgtk-3-0t64 \
    libharfbuzz0b libcairo2 libcairo-gobject2 \
    libatk1.0-0t64 libatk-bridge2.0-0t64 libatspi2.0-0t64 \
    libepoxy0 libfribidi0 libwayland-client0 libwayland-egl1 \
    libwayland-cursor0 libwayland-server0 \
    libselinux1 libmount1 libsepol2 libpcre2-8-0 \
    2>/dev/null || true

echo "==> 解压到 sysroot..."
for f in *.deb; do
    dpkg -x "$f" "$SYSROOT"
done

echo "==> 修复 krb5 符号链接..."
PKG_DIR="$SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig"
# 符号链接指向 mit-krb5/ 子目录但目标文件不存在，直接删除并创建真实文件
rm -f "$PKG_DIR/krb5-gssapi.pc" "$PKG_DIR/krb5.pc" "$PKG_DIR/gssrpc.pc" \
      "$PKG_DIR/kadm-client.pc" "$PKG_DIR/kadm-server.pc" "$PKG_DIR/kdb.pc"
cat > "$PKG_DIR/krb5-gssapi.pc" << 'EOF'
prefix=/usr
exec_prefix=${prefix}
libdir=${prefix}/lib/x86_64-linux-gnu
includedir=${prefix}/include

Name: krb5-gssapi
Description: Kerberos GSSAPI Library
Version: 1.22.1
Libs: -L${libdir} -lgssapi_krb5
Cflags: -I${includedir}
EOF
cat > "$PKG_DIR/krb5.pc" << 'EOF'
prefix=/usr
exec_prefix=${prefix}
libdir=${prefix}/lib/x86_64-linux-gnu
includedir=${prefix}/include

Name: krb5
Description: Kerberos Library
Version: 1.22.1
Libs: -L${libdir} -lkrb5
Cflags: -I${includedir}
EOF

echo "==> 验证 pkg-config..."
export PKG_CONFIG_PATH="$PKG_DIR:$SYSROOT/usr/share/pkgconfig"
for pc in gobject-2.0 glib-2.0 gtk+-3.0 webkit2gtk-4.1 libsoup-3.0 librsvg-2.0 xkbcommon; do
    echo -n "  $pc: "
    pkg-config --exists "$pc" && echo "OK" || echo "MISSING"
done

echo ""
echo "✓ 完成！运行: source apps/desktop/src-tauri/tauri-env.sh && pnpm tauri dev"

#!/bin/sh
# Builds libgit2 for WASI into deps/out, with wasi.diff applied and the
# game's glue (shim.c), for cargo to link into branchwise.wasm. The game
# serves the WASI calls itself (src/vfs.rs), so the page needs nothing.
# Fetches wasi-sdk and libgit2 into deps/ the first time.
set -e
cd "$(dirname "$0")"
SDK_VER=25
LIBGIT2=v1.9.1
LIBGIT2_REV=0060d9cf5666f015b1067129bd874c6cc4c9c7ac
mkdir -p deps
D=$(pwd)/deps
SDK=$D/wasi-sdk-$SDK_VER.0-x86_64-linux
if [ ! -x "$SDK/bin/clang" ]; then
  curl -sSL "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$SDK_VER/wasi-sdk-$SDK_VER.0-x86_64-linux.tar.gz" | tar xz -C "$D"
fi
if [ "$(git -C "$D/libgit2" rev-parse HEAD 2>/dev/null)" != "$LIBGIT2_REV" ]; then
  rm -rf "$D/libgit2"
  git clone -q --depth 1 --branch "$LIBGIT2" https://github.com/libgit2/libgit2.git "$D/libgit2"
  git -C "$D/libgit2" apply "$(pwd)/wasi.diff"
fi
cmake -S "$D/libgit2" -B "$D/build" -G Ninja \
  -DCMAKE_TOOLCHAIN_FILE="$SDK/share/cmake/wasi-sdk.cmake" -DWASI_SDK_PREFIX="$SDK" \
  -DCMAKE_BUILD_TYPE=MinSizeRel -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTS=OFF -DBUILD_CLI=OFF \
  -DBUILD_EXAMPLES=OFF -DUSE_THREADS=OFF -DUSE_HTTPS=OFF -DUSE_SSH=OFF -DUSE_NTLMCLIENT=OFF \
  -DUSE_GSSAPI=OFF -DUSE_AUTH_NEGOTIATE=OFF -DUSE_ICONV=OFF -DUSE_NSEC=OFF \
  -DUSE_SHA1=CollisionDetection -DUSE_SHA256=builtin -DREGEX_BACKEND=builtin \
  -DUSE_HTTP_PARSER=builtin -DUSE_BUNDLED_ZLIB=ON \
  "-DCMAKE_C_FLAGS=-D_WASI_EMULATED_MMAN -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_GETPID" >/dev/null
ninja -C "$D/build" >/dev/null
mkdir -p "$D/out"
cp "$D/build/libgit2.a" "$D/out/"
"$SDK/bin/clang" --target=wasm32-wasip1 --sysroot="$SDK/share/wasi-sysroot" -Os -Wall -Wextra -Wno-missing-field-initializers \
  -I "$D/libgit2/include" -c shim.c -o "$D/out/shim.o"
rm -f "$D/out/libbwshim.a"
"$SDK/bin/llvm-ar" rcs "$D/out/libbwshim.a" "$D/out/shim.o"
for l in c wasi-emulated-mman wasi-emulated-signal wasi-emulated-process-clocks wasi-emulated-getpid; do
  cp "$SDK/share/wasi-sysroot/lib/wasm32-wasip1/lib$l.a" "$D/out/"
done
cp "$SDK/lib/clang/19/lib/wasip1/libclang_rt.builtins-wasm32.a" "$D/out/"

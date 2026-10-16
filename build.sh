#!/bin/sh
# Builds the original games to freestanding WebAssembly (no libc, no JS glue).
# Needs clang and wasm-ld with the wasm32 target (LLVM 15+); the Rust ones
# (folders with a Cargo.toml, on the wg engine from
# wasmga-me/wasmgame-wg) need cargo with the wasm32-unknown-unknown target.
# Function names stay in the modules, so a crash trace names them.
# The engine headers (wg.h, wgfb.h, wgview.h, sdf_font.h) live in
# wasmga-me/wasmgame-wg-c at the commit in wg-c.rev, fetched into wg-c/;
# WG_C=path/to/a/checkout builds against a local copy instead.
set -e
cd "$(dirname "$0")"
# Into the site's public/games, or out/ anywhere else.
if [ -d ../public/games ]; then OUT=../public/games; else OUT=out; fi
GAMES=$(for f in *.c; do basename "$f" .c; done)
# The built .wasm files are committed, so a machine without clang (such as
# Cloudflare Workers Builds) keeps them as they are.
if ! command -v clang >/dev/null 2>&1; then
  for g in $GAMES; do
    [ -f "$OUT/$g/$g.wasm" ] || { echo "clang not found and $OUT/$g/$g.wasm is missing" >&2; exit 1; }
  done
  echo "clang not found; keeping the committed game builds"
  exit 0
fi
engine() {
  [ -n "$WG_C" ] && return
  WG_C=wg-c
  rev=$(cat wg-c.rev)
  [ "$(git -C wg-c rev-parse HEAD 2>/dev/null)" = "$rev" ] && return
  [ -d wg-c/.git ] || git init -q wg-c
  git -C wg-c fetch -q --depth 1 https://github.com/wasmga-me/wasmgame-wg-c "$rev"
  git -C wg-c checkout -q --detach FETCH_HEAD
}
engine
for g in $GAMES; do
  mkdir -p "$OUT/$g"
  clang -I "$WG_C" --target=wasm32 -O2 -ffreestanding -fno-builtin -nostdlib \
    -Wall -Wextra -Wno-unused-function \
    -Wl,--no-entry -Wl,--strip-debug \
    -o "$OUT/$g/$g.wasm" "$g.c"
  echo "built $OUT/$g/$g.wasm ($(wc -c < "$OUT/$g/$g.wasm") bytes)"
done
for dir in */Cargo.toml; do
  g=$(dirname "$dir")
  grep -q cdylib "$dir" || continue # not a game
  if ! command -v cargo >/dev/null 2>&1 || ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
    [ -f "$OUT/$g/$g.wasm" ] || { echo "cargo with wasm32-unknown-unknown not found and $OUT/$g/$g.wasm is missing" >&2; exit 1; }
    echo "keeping the committed $g build"
    continue
  fi
  (cd "$g" && cargo build --quiet --release --target wasm32-unknown-unknown)
  mkdir -p "$OUT/$g"
  python3 tools/wasm-names.py "$g/target/wasm32-unknown-unknown/release/$g.wasm" "$OUT/$g/$g.wasm"
  echo "built $OUT/$g/$g.wasm ($(wc -c < "$OUT/$g/$g.wasm") bytes)"
done

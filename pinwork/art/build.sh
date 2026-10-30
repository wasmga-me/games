#!/bin/sh
# Draws the art (draw.py), renders it (render.mjs, needs sharp) and packs
# it into ../src/art.png and ../src/art.rs with render3d's pack.py (needs
# Pillow).
set -e
cd "$(dirname "$0")"
out=$(mktemp -d)
python3 draw.py "$out"
node render.mjs "$out"
python3 ../../tools/render3d/pack.py "$out/jobs.json" "$out" ../src/art.png ../src/art.rs --colours 256
# pack.py names the jobs file as the source; name the script instead
sed -i 's|from jobs.json|from art/draw.py|' ../src/art.rs
rm -r "$out"

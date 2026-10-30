#!/bin/sh
# Downloads Kenney packs (CC0, https://kenney.nl) into models/<pack>/ for
# render.mjs: sh fetch.sh car-kit cube-pets ...
set -e
cd "$(dirname "$0")"
for p in "$@"; do
  [ -d "models/$p" ] && continue
  jar=$(mktemp)
  id=$(curl -s -c "$jar" -b "$jar" "https://kenney.nl/assets/$p" | grep -o "data-download-id='[0-9a-f]*'" | head -1 | cut -d"'" -f2)
  url=$(curl -s -c "$jar" -b "$jar" -H "Referer: https://kenney.nl/assets/$p" "https://kenney.nl/download-link/$id" |
    python3 -c 'import sys, json; print(json.load(sys.stdin)["url"])')
  rm -f "$jar"
  mkdir -p "models/$p"
  curl -sL "$url" -o "models/$p.zip"
  (cd "models/$p" && unzip -qo "../$p.zip")
  rm "models/$p.zip"
  echo "fetched $p"
done

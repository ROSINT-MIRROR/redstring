#!/usr/bin/env bash
# Regenerates assets/flags/*.png and prints the FLAG_META rows for src/model/flag.rs.
# usage: build-flags.sh <region-flags checkout> <iso-codes iso_3166-1.json>
# needs: magick (ImageMagick 7), oxipng, jq
set -euo pipefail
src="$1"
iso="$2"
out="$(cd "$(dirname "$0")" && pwd)"
side=256

jq -r '."3166-1"[] | "\(.alpha_2)\t\(.name)"' "$iso" | sort | while IFS=$'\t' read -r code name; do
  png="$src/png/$code.png"
  [[ -f "$png" ]] || { echo "missing flag for $code" >&2; exit 1; }
  magick "$png" -resize "${side}x${side}" -strip "PNG32:$out/$code.png"
  printf '    f("%s", "%s", include_bytes!("../../assets/flags/%s.png")),\n' "$code" "${name//\"/\\\"}" "$code"
done
oxipng -q -o max --strip all "$out"/*.png

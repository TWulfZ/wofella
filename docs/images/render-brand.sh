#!/usr/bin/env bash
# Re-render the brand PNGs from their sources: bash docs/images/render-brand.sh
# banner.html and social-preview.html embed Exo 2 and Geist as woff2 data URIs, so no system fonts are involved.
# Needs a Chromium headless shell: set CHROME, or install one with `npx -y playwright install --only-shell chromium`.
set -euo pipefail
dir="$(cd "$(dirname "$0")" && pwd)"
chrome="${CHROME:-$(ls -d "$HOME"/.cache/ms-playwright/chromium_headless_shell-*/*/chrome-headless-shell 2>/dev/null | sort -V | tail -1)}"
[ -x "$chrome" ] || { echo "no chrome-headless-shell found; set CHROME" >&2; exit 1; }

shot() { # source png width height scale
  "$chrome" --no-sandbox --hide-scrollbars --force-device-scale-factor="$5" --window-size="$3,$4" \
    --virtual-time-budget=3000 --screenshot="$dir/$2" "file://$dir/$1" >/dev/null 2>&1
}
# Banner at 2x so it stays crisp at GitHub's ~830px README width; social preview must be exactly 1280x640.
shot banner.html banner.png 1200 300 2
shot social-preview.html social-preview.png 1280 640 1
file "$dir"/banner.png "$dir"/social-preview.png

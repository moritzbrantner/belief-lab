#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/dist}"
TEMPLATE_ROOT="${GITHUB_PAGES_TEMPLATE_ROOT:-$ROOT/.pages-template}"

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$ROOT/site/showcase/index.html" "$OUT/index.html"
cp "$ROOT/site/showcase/site.css" "$OUT/site.css"
cp "$ROOT/site/showcase/site.js" "$OUT/site.js"
cp "$ROOT/site/showcase/demo-data.js" "$OUT/demo-data.js"

if [[ ! -f "$TEMPLATE_ROOT/bin/github-pages-template.mjs" ]]; then
  echo "github-pages-template not found at $TEMPLATE_ROOT" >&2
  echo "Set GITHUB_PAGES_TEMPLATE_ROOT to the exact checked-out template revision." >&2
  exit 2
fi

node "$TEMPLATE_ROOT/bin/github-pages-template.mjs" build \
  --config "$ROOT/site/pages.config.json" \
  --out "$OUT" \
  --augment

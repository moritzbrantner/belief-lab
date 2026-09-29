#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.128" ]]; then
  echo 'Install the matching binding generator: cargo install wasm-bindgen-cli --version 0.2.128 --locked' >&2
  exit 1
fi
cargo build --locked --release -p belief-cli --lib --target wasm32-unknown-unknown
mkdir -p target/pages/pkg target/pages/examples
wasm-bindgen target/wasm32-unknown-unknown/release/belief_cli.wasm --target web --out-dir target/pages/pkg
cp site/index.html site/style.css site/app.js site/translations.js target/pages/
cp -R fixtures/explain target/pages/examples/
cp -R fixtures/evidence-interchange target/pages/examples/
cp -R examples/decisions target/pages/examples/

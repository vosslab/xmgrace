#!/usr/bin/env bash
# Build the browser app, worker, and WebAssembly for GitHub Pages.

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

for required in src/main.ts src/web/worker.ts src/index.html src/style.css; do
	if [ ! -f "$required" ]; then
		echo "ERROR: required build input missing: $required" >&2
		exit 1
	fi
done

wasm-pack build . --release --target web --out-dir generated/wasm --out-name grace --locked
./node_modules/.bin/tsc --noEmit -p tsconfig.json

rm -rf dist
./node_modules/.bin/esbuild src/main.ts src/web/worker.ts \
	--bundle \
	--format=esm \
	--splitting \
	--target=es2020 \
	--platform=browser \
	--loader:.agr=text \
	--minify \
	--sourcemap \
	--entry-names='[name]' \
	--outdir=dist

cp src/index.html src/style.css generated/wasm/grace_bg.wasm COPYRIGHT LICENSE.GPL-2.0-or-later dist/
touch dist/.nojekyll

echo "Built dist/ (GitHub Pages-ready)."

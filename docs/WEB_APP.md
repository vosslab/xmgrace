# Grace web app

The browser port uses the Grace 5.1.25 desktop interface and project format as its reference.
TypeScript handles menus, dialogs, and file selection. The plotting engine and project model
live in a single Rust crate, compiled to WebAssembly and run in a dedicated worker.
Project files and data stay in the browser.

## Build and verify

On macOS, install the build tools with `brew bundle`. Install the stable Rust toolchain and
its browser target with `rustup toolchain install stable --target wasm32-unknown-unknown`.
The app requires Node.js 26 and Rust 1.100 or newer.

```bash
npm ci
./build_github_pages.sh
./run_web_server.sh
```

Run `cargo test --locked` for the native engine and `./check_codebase.sh` for TypeScript.
Install the Playwright browsers with `npx playwright install chromium firefox webkit`.
After building, run `npm run test:playwright` to test the production files under `/xmgrace/`.

## Compatibility

| Workflow | Browser support |
| --- | --- |
| Plot and edit XY data | Two-column ASCII import, linear and logarithmic axes, graph and set appearance, autoscale, zoom, and locator |
| Load existing projects | Multi-graph XY, common symbols and line styles, symmetric and asymmetric XY error bars, and older page and range declarations |
| Save and print | Download an editable AGR or vector SVG, or print through the browser |
| Unsupported settings or graph types | Read-only partial preview with diagnostics and access to the original file |
| Further porting | Classic dialog and shortcut parity, Grace text escapes and font metrics, advanced graphs and axes, data editing, transformations, fitting, the command window, and additional hardcopy formats |

SVG layout and formatting still need detailed visual comparison with native Grace. The
desktop program remains the reference for plotting and numerical behavior.

## Publishing

The Pages build writes a static site to `dist/`. The repository owner can select GitHub
Actions as the Pages source, then dispatch **Deploy GitHub Pages** when ready.
The workflow is manual. The build packages the original Grace copyright and
[GPL-2.0-or-later license](../LICENSE.GPL-2.0-or-later) alongside the application.
The Rust and TypeScript source and build scripts are in the same
[source repository](https://github.com/vosslab/xmgrace).

## Refresh the README screenshot

Start the local preview with `PORT=8787 ./run_web_server.sh`. Install Chromium with
`npx playwright install chromium` and make `cwebp` available on your path.
Then capture and encode the cooling plot from [README.md](../README.md).

```bash
capture_dir=$(mktemp -d)
node tests/playwright/screenshot_web.mjs http://localhost:8787/ "$capture_dir/cooling_curve.png"
cwebp -q 90 "$capture_dir/cooling_curve.png" -o "$capture_dir/cooling_curve.webp"
cp "$capture_dir/cooling_curve.webp" docs/screenshots/cooling_curve.webp
rm "$capture_dir/cooling_curve.png" "$capture_dir/cooling_curve.webp"
rmdir "$capture_dir"
```

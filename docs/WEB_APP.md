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
| Create and edit data | Formula-generated XY, browser text entry, paged spreadsheet creation and editing, set operations, mapped ASCII tables, and numeric variables from NetCDF classic CDF-1 and CDF-2 files |
| Analyze | Expressions with set references and region restrictions, FFT/DFT and windows, histograms, running statistics, differences, integration, splines, regression, nonlinear fitting, correlation, filtering, convolution, geometric transforms, sampling, pruning, and feature extraction |
| Plot and annotate | XY, Fixed, Chart, Polar, and Pie rendering, with solid area fills and baselines for XY sets. Alternate axes, linear/logarithmic/reciprocal/logit scales, calendar and geographic tick formats, drawing objects, and common Grace text escapes. |
| Manage graphs | Create, arrange, overlay, and style graphs, select points, and navigate with the classic toolbar. Locator settings belong to the open browser session. |
| File handling | Select local files for project, data, parameter, and command inputs. Save downloads AGR and parameter files. Export downloads ASCII, SVG, PNG, JPEG, or vector PDF, and browser print uses the current plot. |
| Commands | Run supported Grace plotting and analysis commands in the browser from typed input or a local command file. |
| Unsupported settings | Read-only partial preview with diagnostics and access to the original file |
| Further parity work | Older AGR migrations, exact layout and font metrics, custom ticks and fills, direct canvas editing gestures, more command statements, and native fixture coverage of advanced plots and analysis. |

The desktop program remains the reference for plotting and numerical behavior. The Fourier fixtures
use Grace 5.1.25 with its built-in FFT backend. Full GUI compatibility is tracked in the
[browser parity plan](active_plans/active/classic_grace_browser_parity.md).

## Repository boundaries

The browser app lives under `src/`, with its editable examples in `src/examples/`.
Rust owns the project model and creates blank projects directly. The AGR reader uses a copy
of the Grace 5.1 desktop defaults as an input-format reference for recognizing desktop output.
The build bundles the browser code, worker, WebAssembly, and project examples together.

For native comparisons, install Grace 5.1.25 separately and use `gracebat` to render sample
projects. The inherited desktop implementation is preserved in Git history at commit
`f6a9045e9028277fcf7ad297c4c0fdb766753376`. The upstream project maintains the native
downloads and documentation. [Desktop Grace project](https://plasma-gate.weizmann.ac.il/Grace/)

## Publishing

The Pages build writes a static site to `dist/`. Every successful push to `main` builds, tests,
uploads the artifact, and deploys that commit to GitHub Pages.

With GitHub Actions selected as the Pages source, the repository owner can also open
**Deploy GitHub Pages** in Actions, select **Run workflow**, and choose `main` to rebuild and
publish the selected revision. [Deploy GitHub Pages workflow](https://github.com/vosslab/xmgrace/actions/workflows/deploy-pages.yml)

The build packages the original Grace copyright and
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

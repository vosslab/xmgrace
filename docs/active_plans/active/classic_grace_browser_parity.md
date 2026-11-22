# Plan: Classic Grace browser parity

## Context

Grace is becoming a browser-only scientific plotting application. Grace 5.1.25 is the
behavioral and AGR-format reference. This repository retains no desktop product code and does
not aim to ship one.

The browser already has an editable XY workflow, AGR open and save, SVG output, browser print,
and a modern CSS presentation. The visual refresh must retain Grace menu names, toolbar labels,
and control placement. The active implementation restores the classic Data workflow first while
building the project-model boundaries needed for full GUI parity.

## Objectives

- Deliver the classic Grace plotting and data-analysis workflows through an accessible web GUI.
- Preserve Grace menu names, toolbar labels, and control placement while using modern CSS.
- Keep project files and imported data local to the browser and save output through downloads.
- Maintain AGR interoperability with Grace 5.1.25 and document intentional browser limits.
- Build, test, and publish successful pushes to `main`, while retaining a manual rebuild route.

## Design philosophy

This plan follows the repository principle **Fix the design, not the symptom**. Rust owns the
typed project, data, analysis, AGR codecs, and renderer. TypeScript owns menus, dialogs, local
file selection, and browser downloads. A typed command contract is shared by dialogs and the
browser command window, so an operation has one implementation and one error model.

The evidence strategy pins native numerical and visual fixtures to Grace 5.1.25. A mismatch is
investigated in the Rust operation or renderer before a feature is described as compatible.

## Scope

- Restore classic GUI organization, labels, toolbar controls, dialogs, and canvas interactions.
- Implement browser-safe data creation, editing, transformation, analysis, rendering, and export.
- Extend the typed project model and AGR codec only as renderer and editor support land.
- Add browser-safe local file reload, preferences, scripting, and supported format workflows.
- Retain current browser print and add direct browser-controlled output formats where feasible.

## Non-goals

- Reintroduce a desktop xmgrace build, X11 UI, Motif UI, or native distribution.
- Execute shell commands, native shared libraries, pipes, or external editors from a web page.
- Claim exact parity for a feature before fixture evidence establishes it.
- Replace the browser-oriented Pages delivery with a server-side application.

## Current state summary

The browser implements the classic menu paths and toolbar actions, local file selection and
downloads, numeric data creation and editing, and the main Data transformations. Native
Grace 5.1.25 fixtures cover Fourier magnitude, phase, complex coefficients, and Hamming
windowing. Numerical and visual reference coverage for the remaining operations is still open.

This inventory records the implemented features and the remaining work.

| Area | Implemented in the browser | Remaining parity work |
| --- | --- | --- |
| File | New, local upload, AGR download, revert, print, page settings, and parameter files. | Older AGR format migrations and complete project metadata. |
| Edit data | Formula, spreadsheet, text, and mapped table creation. Set management, graph arrangement and overlay, regions, local reload, locator controls, and preferences. | Alphanumeric data and full classic dialog options. |
| Data | Classic transformation menu including expression evaluation, Fourier, regression, interpolation, filtering, and feature extraction. ASCII and NetCDF classic imports. | More native fixtures per operation and remaining options and file variants. |
| Plot | Graph, set, axis and page editors, alternate axes, tick formats, and parameter files. | Custom ticks, full legend and axis layouts, value labels, set patterns, and exact dialog options. |
| View and toolbar | Draw, zoom, autoscale, scroll, AutoT, AutoO, ZX, ZY, AX, AY, PZ, Pu, Po, Cy, Exit, and per-graph world stack. | Complete keyboard and pointer behavior audit and persisted locator and view state. |
| Canvas and objects | Point explorer, graph focus, regions, and editors for text, lines, arrows, boxes, and ellipses. | Direct editing gestures, annotation patterns, and timestamp editing. |
| Text and fonts | Common Grace escapes, Symbol/Greek, and a font tool. | Advanced typesetting escapes, custom font maps, and font-metric visual calibration. |
| Graph kinds | XY, Chart, Polar, Pie, and Fixed, with bars, HILO, vectors, box plots, variable size, and variable color. | Visual fixtures and option-level comparisons for each graph and set type. |
| Automation | Browser command window, selected command files, and atomic execution of a supported subset. | Remaining browser-safe statements and expression functions. |
| Publishing | AGR, ASCII, SVG, PNG, JPEG, PDF, and browser print. | Cross-format visual comparisons and documented legacy device equivalents. |

## Architecture boundaries and ownership

### Mapping (milestones -> components)

| Milestone | Component | Review boundary |
| --- | --- | --- |
| M0 | Build workflow and fixture corpus | Successful `main` pushes validate and publish; manual rebuild remains available. |
| M1 | Project, dataset, and expression core | Rust validates input before any mutation and preserves stable graph/set IDs. |
| M2 | Numerical analysis | Rust functions consume typed rows and return complete derived rows before result-set allocation. |
| M3 | AGR, layout, renderer, text, and objects | Typed supported settings round-trip. Unknown semantics remain read-only with original download. |
| M4 | Browser interaction and dialogs | TypeScript renders controls and requests typed worker commands. It never owns project rows. |
| M5 | File, script, preferences, and exports | Browser capabilities are explicit and no command obtains host execution authority. |

### File scope

| File or directory | Intended change | Canonical source |
| --- | --- | --- |
| `rust/src/` | Typed model, AGR codec, analysis, SVG layout, command execution, and Wasm adapter. | Grace 5.1.25 fixtures and browser contracts. |
| `src/` and `src/web/` | Classic menu and toolbar surface, dialogs, local file selection, download workflows, and browser interaction. | Rust command and summary types. |
| `tests/` | Numerical, AGR, rendering, and browser workflow tests. | Stable user-visible behavior. |
| `.github/workflows/` | Build, test, and publish successful `main` pushes, with a manual rebuild route. | `build_github_pages.sh`. |
| `docs/` | Compatibility matrix, active plan, release history, changelog, usage, and screenshot evidence. | Verified implementation behavior. |

## Milestone plan

| M | Title | Summary | Goal |
| --- | --- | --- | --- |
| M0 | Reference and automation | Pin Grace 5.1.25 fixtures and restore `main` push validation and publication. | Every parity claim has a reproducible reference. |
| M1 | Data foundations | Finish formula creation, spreadsheet editing, table mapping, set operations, and ASCII export. | A user can create, edit, save, and reopen scientific data without leaving the browser. |
| M2 | Analysis | Finish transformations, fitting, feature extraction, and numerical parity evidence. | Derived data becomes editable Grace sets with useful diagnostics. |
| M3 | Plot and project fidelity | Complete graph/set/axis/annotation models, renderer paths, AGR round trips, and classic canvas workflow. | Representative desktop AGR files can be edited without semantic loss. |
| M4 | Browser-safe feature equivalents | Complete local reload, preferences, output, NetCDF, scripts, and supported browser replacements. | Browser workflows replace desktop workflows that do not depend on native host access. |
| M5 | Parity close-out | Audit every menu and toolbar control, update compatibility documentation, and close supported gaps. | The web GUI provides the intended full Grace workflow surface. |

### Milestone M0

- Depends on: none.
- Deliverables: Grace 5.1.25 fixture provenance, native and Wasm comparison harnesses, and the
  `main` publication workflow with a manual rebuild route.
- Entry criteria: local `gracebat` reference is available.
- Exit criteria: a successful `main` push runs build and tests before publishing its artifact.
  Manual dispatch can rebuild and republish `main`.
- Parallel-plan ready: yes. Fixture collection is independent of GUI implementation.

### Milestone M1

- Depends on: M0 numerical and AGR fixtures establish expected inputs.
- Deliverables: formula generation including the sine-wave recipe, expression evaluation, a
  paged spreadsheet editor, mapped ASCII import, set operations, and ASCII download.
- Entry criteria: project data remains owned by Rust and worker commands are typed.
- Exit criteria: formula, spreadsheet, and import flows retain data after AGR save and reopen.
- Parallel-plan ready: yes. Parser, data model, and dialogs have clear contract boundaries.

### Milestone M2

- Depends on: M1 typed rows and result-set allocation.
- Deliverables: DFT/FFT, windows, histograms, rolling statistics, differences, integration,
  interpolation, regression, nonlinear fitting, correlation, filters, convolution, geometric
  transforms, sampling, pruning, and feature extraction.
- Entry criteria: invalid inputs leave the source project unchanged.
- Exit criteria: each enabled operation has native and Wasm fixtures, browser dialog coverage,
  and stated output-set semantics.
- Parallel-plan ready: yes. Independent operation families share typed input and output contracts.

### Milestone M3

- Depends on: M1 data types and M2 operations for representative input.
- Deliverables: graph kinds, special sets, full axis properties, graph arrangement, annotations,
  regions, point explorer, object editor, world stack, rich text, and parameter files.
- Entry criteria: AGR statements are represented by typed owners or safely remain read-only.
- Exit criteria: supported projects round-trip through edit/save/open and visual fixtures meet
  documented geometry and text tolerances.
- Parallel-plan ready: partial. Model and AGR work precede dialog exposure for each feature.

### Milestone M4

- Depends on: M3 model and rendering support.
- Deliverables: file reload with user file selection, local preference persistence and exchange,
  NetCDF variable selection, PNG/JPEG output, browser print/PDF guidance, and script execution.
- Entry criteria: each workflow is expressible through typed worker commands.
- Exit criteria: browser tests prove local bytes stay local, downloads use the chosen filename,
  and scripts cannot execute host code.
- Parallel-plan ready: yes after command and project contracts stabilize.

### Milestone M5

- Depends on: M0 through M4.
- Deliverables: menu-by-menu compatibility audit, documented exceptions, release documentation,
  and visual evidence.
- Entry criteria: each prior milestone has its intended test evidence.
- Exit criteria: every classic menu item is supported, mapped to a browser equivalent, or clearly
  documented as host-specific.
- Parallel-plan ready: no. This is an integration and evidence pass.

## Test and verification strategy

- Run `cargo fmt --check`, `cargo test --locked`, and Clippy with warnings denied for Rust changes.
- Build the `wasm32-unknown-unknown` package and exercise every deterministic fixture through the
  native core and browser worker.
- Use Grace 5.1.25 built-in FFT behavior as the baseline for FFT and DFT normalization, window,
  bin, and phase fixtures.
- Add permanent tests for stable user workflows, including formula to FFT to AGR download and
  reopen, local file selection, set editing, graph arrangement, and supported export bytes.
- Run Playwright against production files in Chromium, Firefox, and WebKit. A failed browser test
  blocks its related feature until the UI and worker contract agree.
- Record visual comparisons for representative titles, labels, ticks, legends, error bars, charts,
  polar plots, pie plots, and rich text. A material mismatch remains a compatibility gap.

## Migration and compatibility policy

New projects use the typed Rust model. Imported AGR projects remain editable only when every
statement affected by an edit has a typed, behavior-preserving owner. Unsupported projects remain
read-only previews with diagnostics and an original-file download. A newly supported graph or set
type requires parser, model, renderer, editor, serializer, and fixture coverage before it changes
this policy.

Local import actions invoke the browser file picker. Users select files to upload into the app;
the app does not retain a filesystem path or a host file handle after the selected bytes are read.
Save and export actions download files. Reload asks the user to select the file again.

## Risk register

| Risk | Impact | Trigger | Owner | Mitigation |
| --- | --- | --- | --- | --- |
| Numeric mismatch | A plausible plot has incorrect science. | Reference fixture differs. | Analysis component | Block feature promotion and correct normalization, units, or input rules. |
| AGR data loss | Editing drops or changes desktop semantics. | Unsupported statement or setting appears. | Codec component | Preserve original and use read-only preview until typed support exists. |
| GUI drift | Modern styling changes familiar Grace controls. | Screenshot or menu audit differs. | Browser UI component | Restore classic labels, order, and placement while retaining CSS theme. |
| Browser resource exhaustion | Large tables or scripts stall the app. | Bounds or timing test fails. | Worker component | Validate work before mutation, bound execution, and add cancellation where needed. |
| Scope uncertainty | Native-only behavior is treated as absent without a web equivalent. | Feature audit reaches host dependency. | Parity audit | Document the browser equivalent or explicit host-specific exception. |

## Rollout and release checklist

- [ ] Verify a `main` push publication on GitHub Actions.
- [x] Verify manual dispatch of `e033f18` rebuilt and published the live Pages artifact with the
  restored classic toolbar.
- [x] Refresh the README screenshot with the restored toolbar and cooling plot.
- [x] Document the available workflows and known limitations in [Browser compatibility](../../WEB_APP.md#compatibility).
- [x] Confirm saved AGR, PNG, and PDF in clean Chromium, Firefox, and WebKit contexts.

## Documentation close-out requirements

- Active plan / progress tracker: this file remains active until M5 closes.
- `docs/CHANGELOG.md` entry: record each delivered user-visible workflow.
- Archive / closure notes: move this plan to `docs/archive/` when the parity audit is complete.
- Temporary verification closeout: remove scratch fixtures and promote only stable user contracts.

## Open questions and decisions needed

- Full visual parity requires a browser-owned font-metrics decision. The renderer owner compares
  browser fonts with Grace fixtures and records the selected metric and fallback policy.
- NetCDF classic CDF-1 and CDF-2 use a Rust decoder for one-dimensional numeric variables.
  Other NetCDF variants remain outside current support.
- The Rust core uses a radix-2 FFT and a separate DFT. Native Grace fixtures verify normalization
  and representative output before either implementation is promoted as compatible.
- Native-only printer devices, pipes, shared libraries, and external editors remain browser
  exceptions. The parity audit names their browser equivalents where a user workflow exists.

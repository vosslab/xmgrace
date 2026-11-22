## 2026-11-22

### Additions and New Features

- Added formula, spreadsheet, and text-editor data creation, mapped table and NetCDF classic
  import, and ASCII export.
- Added Fourier transforms, histograms, smoothing, calculus, interpolation, fitting, filtering,
  and other Grace data tools.
- Added graph management, more graph and set presentations, drawing objects, regions,
  parameter files, a browser command window, and PNG, JPEG, and PDF export.
- Added alternate axes, calendar and geographic tick labels, locator fixed-point controls,
  classic arrow heads, and XY area fills, baselines, and drop lines.

### Behavior or Interface Changes

- Modernized the browser controls and dialogs, with a wrapping toolbar on narrow screens.
- Restored classic toolbar labels and actions, and grouped the Data menu by classic workflow.

### Fixes and Maintenance

- Removed the inherited desktop C build, vendored libraries, and bundled manuals.
- Made the web app own its blank projects and editable examples while preserving AGR interoperability.
- Linked the live GitHub Pages app in the README and refreshed the screenshot.
- Restored GitHub Pages build checks on pushes to `main`, with publication on manual dispatch.
- Used ASCII escapes for Greek plot labels and submenu chevrons, and moved axis rendering
  into a separate Rust module to meet the source line limit.

## 2026-11-21

### Added

- Added a browser-based Grace XY editor with a Rust WebAssembly worker, AGR import and export, and a manual GitHub Pages build.
- Added Homebrew build prerequisites and native Rust and browser workflow tests.
- Renamed the root GPL license to use its SPDX identifier.
- Added a README with the local browser quick start, a worked cooling plot, and a reproducible screenshot.

### Fixes and Maintenance

- Synchronized shared style guides, tests, and repository support files from the starter template.
- Synchronized shared style guides, tests, and repository support files from the starter template.

import coolingCurve from "./examples/cooling.agr";
import measurementErrors from "./examples/measurement_errors.agr";
import { advancedDialogs } from "./web/advanced_dialogs";
import { appearanceDialogs } from "./web/appearance_dialogs";
import { commandDialog } from "./web/command_dialog";
import {
  closeDialog,
  element,
  field,
  formText,
  input,
  paragraph,
  select,
  showDialog,
} from "./web/dialog";
import {
  dataSetsDialog,
  differenceDialog,
  evaluateDialog,
  formulaDialog,
  fourierDialog,
  histogramDialog,
  integrationDialog,
  interpolationDialog,
  regressionDialog,
  runningDialog,
  setOperationsDialog,
} from "./web/data_dialogs";
import { importTableDialog } from "./web/import_dialog";
import { newSpreadsheetDialog, spreadsheetDialog } from "./web/data_table";
import { engine } from "./web/engine";
import { figureExportDialog } from "./web/figure_export";
import { graphDialogs } from "./web/graph_dialogs";
import { guidedDemo } from "./web/guided_demo";
import { inspectorDialogs } from "./web/inspector_dialogs";
import { navigation } from "./web/navigation";
import { netcdfDialog } from "./web/netcdf_dialog";
import { objectDialogs } from "./web/object_dialogs";
import { preferences } from "./web/preferences";
import { regionDialogs } from "./web/region_dialogs";
import type { DataContext } from "./web/data_dialogs";
import type { AxisScale, Command, Graph, Project, Request } from "./web/protocol";

const canvas = element("canvas");
const plot = element<HTMLImageElement>("plot");
const status = element("status-bar");
const graphSelect = element<HTMLSelectElement>("graph-select");
const projectFile = element<HTMLInputElement>("project-file");
const dataFile = element<HTMLInputElement>("data-file");
const tableFile = element<HTMLInputElement>("table-file");
const reloadFile = element<HTMLInputElement>("reload-file");
const netcdfFile = element<HTMLInputElement>("netcdf-file");
const parametersFile = element<HTMLInputElement>("parameters-file");
const controls = Array.from(document.querySelectorAll<HTMLButtonElement>("button[data-action]"));
const menus = Array.from(document.querySelectorAll<HTMLDetailsElement>("nav > details"));
const editControls = controls.filter((button) => button.hasAttribute("data-edit"));
const examples = [
  { filename: "cooling.agr", title: "Cooling curve", source: coolingCurve },
  {
    filename: "measurement_errors.agr",
    title: "Calibration measurements with error bars",
    source: measurementErrors,
  },
] as const;
const sessionDemo = new URLSearchParams(window.location.search).get("demo") === "1";

let project: Project | undefined;
let currentGraph = 0;
let currentSet = 0;
let projectName = "Untitled.agr";
let plotUrl = "";
let original: File | undefined;
let checkpoint: Blob | undefined;
let edited = false;
let active = false;
let reloadSelection: { graph: number; set: number } | undefined;

const graph = (): Graph | undefined => project?.graphs[currentGraph];
const editable = (): boolean => project !== undefined && project.diagnostics.length === 0;
const writeStatus = (message: string): void => {
  status.textContent = message;
};
const text = (value: FormDataEntryValue | string | null): string =>
  typeof value === "string" ? value : "";
const unquote = (value: string): string => value.replace(/^"|"$/g, "");

function refreshControls(): void {
  for (const control of editControls) control.disabled = active || !editable();
  for (const name of ["save", "save-as", "print", "svg"]) {
    const control = controls.find((button) => button.dataset["action"] === name);
    if (control && !project) control.disabled = true;
  }
  element("project-title").textContent = `Grace: ${projectName}${edited ? " *" : ""}`;
}

async function run(request: Request): Promise<void> {
  if (active) return;
  active = true;
  refreshControls();
  try {
    const response = await engine(request);
    if (response.type !== "state") throw new Error("Expected a Grace project.");
    project = response.project;
    if (
      request.type === "apply" ||
      request.type === "import" ||
      request.type === "importMapped" ||
      request.type === "reloadData" ||
      request.type === "importNetcdf" ||
      request.type === "loadParameters" ||
      request.type === "script"
    )
      edited = true;
    if (!graph()) currentGraph = Number(Object.keys(project.graphs)[0] ?? 0);
    graphSelect.replaceChildren(
      ...Object.keys(project.graphs).map((id) => new Option(`G${id}`, id)),
    );
    graphSelect.value = String(currentGraph);
    if (!graph()?.sets[currentSet]) currentSet = Number(Object.keys(graph()?.sets ?? {})[0] ?? 0);
    plotNavigation.sync();
    canvas.style.setProperty("--page-ratio", String(project.page[0] / project.page[1]));
    const previous = plotUrl;
    plotUrl = URL.createObjectURL(new Blob([response.svg], { type: "image/svg+xml" }));
    plot.src = plotUrl;
    if (previous) URL.revokeObjectURL(previous);
    element("diagnostic-banner").hidden = project.diagnostics.length === 0;
    writeStatus(
      project.diagnostics.length > 0 ? "Some settings could not be plotted." : description(),
    );
  } finally {
    active = false;
    refreshControls();
  }
}

function description(): string {
  const count = Object.values(graph()?.sets ?? {}).filter((set) => set.count > 0).length;
  return `G${currentGraph}  |  ${count} ${count === 1 ? "set" : "sets"}`;
}

async function apply(command: Command): Promise<void> {
  await run({ type: "apply", command });
}

function dataContext(): DataContext {
  return {
    graph: currentGraph,
    currentSet,
    selected: graph,
    apply,
    regions: project?.regions ?? {},
  };
}

function report(error: unknown): void {
  writeStatus(error instanceof Error ? error.message : "Grace could not complete the operation.");
}

function download(blob: Blob, filename: string): void {
  const anchor = document.createElement("a");
  const url = URL.createObjectURL(blob);
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
}

async function load(file: File): Promise<void> {
  await run({ type: "open", bytes: await file.arrayBuffer() });
  plotNavigation.reset();
  projectName = file.name;
  original = file;
  checkpoint = file;
  edited = false;
  refreshControls();
}

async function save(filename: string): Promise<void> {
  const response = await engine({ type: "save" });
  if (response.type !== "save") throw new Error("Grace did not return project bytes.");
  checkpoint = new Blob([response.bytes], { type: "application/octet-stream" });
  projectName = filename;
  edited = false;
  download(checkpoint, filename);
  refreshControls();
  writeStatus(`Saved ${filename}`);
}

function confirmDiscard(action: () => Promise<void> | void): void {
  if (!edited) {
    void Promise.resolve(action()).catch(report);
    return;
  }
  showDialog("Unsaved changes", [paragraph("Discard the changes to this project?")], action);
}

projectFile.addEventListener("change", () => {
  const file = projectFile.files?.[0];
  projectFile.value = "";
  if (file) void load(file).catch(report);
});

reloadFile.addEventListener("change", () => {
  const file = reloadFile.files?.[0];
  const selected = reloadSelection;
  reloadFile.value = "";
  reloadSelection = undefined;
  if (file && selected)
    void file
      .arrayBuffer()
      .then((bytes) => run({ type: "reloadData", ...selected, bytes }))
      .catch(report);
});

dataFile.addEventListener("change", () => {
  const file = dataFile.files?.[0];
  dataFile.value = "";
  if (file)
    void file
      .arrayBuffer()
      .then((bytes) => run({ type: "import", graph: currentGraph, bytes }))
      .catch(report);
});

tableFile.addEventListener("change", () => {
  const file = tableFile.files?.[0];
  tableFile.value = "";
  if (!file) return;
  void importTableDialog(file, {
    graph: currentGraph,
    import: (request) => run(request),
  }).catch(report);
});

netcdfFile.addEventListener("change", () => {
  const file = netcdfFile.files?.[0];
  netcdfFile.value = "";
  if (file) void netcdfDialog(file, currentGraph, run).catch(report);
});

parametersFile.addEventListener("change", () => {
  const file = parametersFile.files?.[0];
  parametersFile.value = "";
  if (!file) return;
  const graph = currentGraph;
  void file
    .arrayBuffer()
    .then((bytes) => run({ type: "loadParameters", graph, bytes }))
    .catch(report);
});

function selectGraph(id: number): void {
  currentGraph = id;
  currentSet = Number(Object.keys(graph()?.sets ?? {})[0] ?? 0);
  graphSelect.value = String(id);
  writeStatus(description());
  plotNavigation.sync();
}

graphSelect.addEventListener("change", () => selectGraph(Number(graphSelect.value)));

function dialogButton(label: string, action: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.addEventListener("click", action);
  return button;
}

function axisDialog(axis: "x" | "y" = "x"): void {
  const selected = graph();
  if (!selected) return;
  const index = axis === "x" ? 0 : 1;
  const settings = selected.settings;
  const lower = selected.world[index] ?? 0;
  const upper = selected.world[index + 2] ?? 1;
  const axisSelect = select(
    "axis",
    [
      ["x", "X axis"],
      ["y", "Y axis"],
    ],
    axis,
  );
  axisSelect.addEventListener("change", () => axisDialog(axisSelect.value === "y" ? "y" : "x"));
  const scale = settings[`${axis}axes scale`]?.toLowerCase() ?? "normal";
  showDialog(
    "Axis properties",
    [
      field("Edit", axisSelect),
      field("Start", input("min", String(lower), "number")),
      field("Stop", input("max", String(upper), "number")),
      field(
        "Scale",
        select(
          "scale",
          [
            ["normal", "Normal"],
            ["logarithmic", "Logarithmic"],
            ["reciprocal", "Reciprocal"],
            ["logit", "Logit"],
          ],
          scale,
        ),
      ),
      field("Label", input("label", unquote(settings[`${axis}axis label`] ?? ""))),
      field("Major spacing", input("major", settings[`${axis}axis tick major`] ?? "0.2", "number")),
      dialogButton("More axis settings...", () => appearance.axis(axis)),
    ],
    (data) =>
      apply({
        type: "axis",
        graph: currentGraph,
        axis,
        min: Number(data.get("min")),
        max: Number(data.get("max")),
        label: text(data.get("label")),
        major: Number(data.get("major")),
        scale: formText(data, "scale") as AxisScale,
      }),
  );
}

const indexed = (labels: string[]): [string, string][] =>
  labels.map((label, index) => [String(index), label]);
const colors = indexed([
  "White",
  "Black",
  "Red",
  "Green",
  "Blue",
  "Yellow",
  "Brown",
  "Grey",
  "Violet",
  "Cyan",
  "Magenta",
  "Orange",
  "Indigo",
  "Maroon",
  "Turquoise",
  "Dark green",
]);
const symbols = indexed([
  "None",
  "Circle",
  "Square",
  "Diamond",
  "Triangle up",
  "Triangle left",
  "Triangle down",
  "Triangle right",
  "Plus",
  "Cross",
  "Splatter",
  "Character",
]);
const lines = indexed(["None", "Straight", "Left stairs", "Right stairs"]);

function setDialog(): void {
  const sets = Object.entries(graph()?.sets ?? {}).filter(([, set]) => set.count > 0);
  if (!sets.length) {
    showDialog("Set appearance", [paragraph("Import data to create a set.")]);
    return;
  }
  const [id, set] = sets.find(([id]) => Number(id) === currentSet) ?? sets[0]!;
  const picker = select(
    "set",
    sets.map(([id, set]) => [
      id,
      `G${currentGraph}.S${id}  ${set.kind.toUpperCase()}  (${set.count} points)`,
    ]),
    id,
  );
  picker.addEventListener("change", () => {
    currentSet = Number(picker.value);
    setDialog();
  });
  showDialog(
    "Set appearance",
    [
      field("Select set", picker),
      field("Legend", input("label", unquote(set.settings["legend"] ?? ""))),
      field("Color", select("color", colors, set.settings["line color"] ?? "1")),
      field("Symbol", select("symbol", symbols, set.settings["symbol"] ?? "0")),
      field("Line type", select("line", lines, set.settings["line type"] ?? "1")),
      dialogButton("More set settings...", () => appearance.set()),
      field(
        "Hidden",
        select(
          "hidden",
          [
            ["false", "No"],
            ["true", "Yes"],
          ],
          set.settings["hidden"] ?? "false",
        ),
      ),
    ],
    (data) =>
      apply({
        type: "set",
        graph: currentGraph,
        set: Number(id),
        label: text(data.get("label")),
        color: Number(data.get("color")),
        symbol: Number(data.get("symbol")),
        line: Number(data.get("line")),
        hidden: data.get("hidden") === "true",
      }),
  );
}

function graphDialog(): void {
  const settings = graph()?.settings;
  if (!settings) return;
  showDialog(
    "Graph appearance",
    [
      field("Title", input("title", unquote(settings["title"] ?? ""))),
      field("Subtitle", input("subtitle", unquote(settings["subtitle"] ?? ""))),
      dialogButton("More graph settings...", () => appearance.graph()),
      dialogButton("Viewport...", () => graphActions["viewport"]?.()),
    ],
    (data) =>
      apply({
        type: "graph",
        graph: currentGraph,
        title: text(data.get("title")),
        subtitle: text(data.get("subtitle")),
      }),
  );
}

const settings = preferences(sessionDemo);
const plotNavigation = navigation({
  graph,
  project: () => project,
  activeGraph: () => currentGraph,
  editable,
  apply,
  report,
  selectGraph,
  preferences: settings.values,
});

const graphActions = graphDialogs({
  project: () => project,
  activeGraph: () => currentGraph,
  apply,
});
const appearance = appearanceDialogs({
  graph,
  activeGraph: () => currentGraph,
  activeSet: () => currentSet,
  apply,
});
const demo = guidedDemo();

function resetProject(): void {
  confirmDiscard(async () => {
    await run({ type: "new" });
    original = undefined;
    checkpoint = undefined;
    edited = false;
    projectName = "Untitled.agr";
    plotNavigation.reset();
    refreshControls();
  });
}

function exportFigure(format: "png" | "jpeg" | "pdf"): void {
  if (!project) return;
  const { page } = project;
  figureExportDialog({
    format,
    svgUrl: () => plotUrl,
    filename: () => projectName,
    download,
    page: () => page,
  });
}

const actions: Record<string, () => void | Promise<void>> = {
  ...advancedDialogs({ graph, activeGraph: () => currentGraph, apply }),
  ...graphActions,
  ...plotNavigation.actions,
  preferences: settings.dialog,
  ...objectDialogs({ project: () => project, activeGraph: () => currentGraph, apply }),
  ...inspectorDialogs({
    project: () => project,
    activeGraph: () => currentGraph,
    activeSet: () => currentSet,
    apply,
    download,
  }),
  ...regionDialogs({ project: () => project, activeGraph: () => currentGraph, apply }),
  commands: () => commandDialog((input) => run({ type: "script", graph: currentGraph, input })),
  "guided-demo": demo.open,
  new: resetProject,
  exit: resetProject,
  open: () => confirmDiscard(() => projectFile.click()),
  revert: () =>
    confirmDiscard(async () => {
      if (!checkpoint) return;
      await run({ type: "open", bytes: await checkpoint.arrayBuffer() });
      plotNavigation.reset();
      edited = false;
      refreshControls();
    }),
  save: () => save(projectName),
  "save-as": () =>
    showDialog("Save as", [field("File name", input("name", projectName))], (data) => {
      const name = text(data.get("name")).trim();
      if (!name || /[\\/]/.test(name)) throw new Error("Enter a file name.");
      return save(name.toLowerCase().endsWith(".agr") ? name : `${name}.agr`);
    }),
  import: () => dataFile.click(),
  "reload-data": () => {
    if (!graph()?.sets[currentSet]) {
      showDialog("Hot links", [paragraph("Select an existing data set first.")]);
      return;
    }
    showDialog(
      "Hot links",
      [paragraph(`Choose a local file to replace the points in G${currentGraph}.S${currentSet}.`)],
      () => {
        reloadSelection = { graph: currentGraph, set: currentSet };
        reloadFile.click();
      },
    );
  },
  "import-table": () => tableFile.click(),
  "import-netcdf": () => netcdfFile.click(),
  "load-parameters": () => parametersFile.click(),
  "save-parameters": async () => {
    const response = await engine({ type: "saveParameters", graph: currentGraph });
    if (response.type !== "parameters") throw new Error("Grace did not return graph parameters.");
    download(
      new Blob([response.bytes], { type: "text/plain" }),
      projectName.replace(/\.agr$/i, "") + ".par",
    );
  },
  "data-operations": () => setOperationsDialog(dataContext()),
  "set-operations": () => setOperationsDialog(dataContext(), "sets"),
  evaluate: () => evaluateDialog(dataContext()),
  fourier: () => fourierDialog(dataContext()),
  histogram: () => histogramDialog(dataContext()),
  running: () => runningDialog(dataContext()),
  difference: () => differenceDialog(dataContext()),
  "seasonal-difference": () => differenceDialog(dataContext(), true),
  integration: () => integrationDialog(dataContext()),
  interpolation: () => interpolationDialog(dataContext()),
  regression: () => regressionDialog(dataContext()),
  "export-ascii": () => {
    const available = Object.entries(graph()?.sets ?? {}).filter(([, set]) => set.count > 0);
    if (!available.length) {
      showDialog("Export ASCII", [paragraph("Create or import a data set first.")]);
      return;
    }
    const graphId = currentGraph;
    const options = available.map(([id, set]): [string, string] => [
      id,
      `G${graphId}.S${id} (${set.count} points)`,
    ]);
    showDialog(
      "Export ASCII",
      [field("Set", select("set", options, String(currentSet)))],
      async (data) => {
        const set = Number(data.get("set"));
        const response = await engine({ type: "exportAscii", graph: graphId, set });
        if (response.type !== "ascii") throw new Error("Grace did not return ASCII data.");
        download(
          new Blob([response.bytes], { type: "text/plain;charset=utf-8" }),
          `${projectName.replace(/\.agr$/i, "")}-g${graphId}s${set}.dat`,
        );
      },
    );
  },
  png: () => exportFigure("png"),
  jpeg: () => exportFigure("jpeg"),
  pdf: () => exportFigure("pdf"),
  svg: async () => {
    const response = await fetch(plotUrl);
    download(await response.blob(), projectName.replace(/\.agr$/i, "") + ".svg");
  },
  original: () => {
    if (original) download(original, original.name);
  },
  print: () => window.print(),
  "print-setup": () =>
    showDialog("Print setup", [
      paragraph("Choose the printer and paper options in your browser's Print dialog."),
    ]),
  autoscale: () => apply({ type: "autoscale", graph: currentGraph, axes: "xy" }),
  "autoscale-x": () => apply({ type: "autoscale", graph: currentGraph, axes: "x" }),
  "autoscale-y": () => apply({ type: "autoscale", graph: currentGraph, axes: "y" }),
  "autoscale-dialog": () =>
    showDialog(
      "Autoscale graphs",
      [
        field(
          "Autoscale",
          select(
            "axes",
            [
              ["xy", "X and Y axes"],
              ["x", "X axis"],
              ["y", "Y axis"],
            ],
            "xy",
          ),
        ),
      ],
      (data) => {
        const axis = data.get("axes");
        return apply({
          type: "autoscale",
          graph: currentGraph,
          axes: axis === "x" || axis === "y" ? axis : "xy",
        });
      },
    ),
  graph: graphDialog,
  set: setDialog,
  axis: () => axisDialog(),
  sets: () =>
    dataSetsDialog(
      dataContext(),
      () => formulaDialog(dataContext()),
      (set) => {
        currentSet = set;
        const selected = graph()?.sets[String(set)];
        if (!selected) {
          showDialog("Data sets", [paragraph("Select an existing data set first.")]);
          return;
        }
        void spreadsheetDialog({ graph: currentGraph, set, data: selected, apply });
      },
      () => newSpreadsheetDialog({ graph: currentGraph, apply }),
      () => tableFile.click(),
    ),
  diagnostics: () => {
    const reports = project?.console ?? [];
    const warnings =
      project?.diagnostics.map(({ line, message }) =>
        paragraph(line === null ? message : `Line ${line}: ${message}`),
      ) ?? [];
    const controls: HTMLElement[] = [...warnings, ...reports.map(paragraph)];
    if (!controls.length) controls.push(paragraph(status.textContent ?? ""));
    if (reports.length) {
      controls.push(
        dialogButton("Download results...", () =>
          download(
            new Blob([reports.join("\n") + "\n"], { type: "text/plain" }),
            "grace-results.txt",
          ),
        ),
      );
    }
    showDialog("Console", controls);
  },
  about: () =>
    showDialog("About Grace", [
      paragraph("Grace - GRaphing, Advanced Computation and Exploration of data"),
      paragraph("Based on Grace 5.1.25. Use Help to find the source code and license terms."),
    ]),
  examples: () => {
    const example = select(
      "example",
      examples.map(({ filename, title }) => [filename, title]),
      examples[0].filename,
    );
    const fields = [field("Project", example)];
    showDialog(
      "Examples",
      edited
        ? [paragraph("Opening an example discards unsaved changes to this project."), ...fields]
        : fields,
      async (data) => {
        const selection = examples.find(({ filename }) => filename === data.get("example"));
        if (!selection) throw new Error("Select an example project.");
        await load(new File([selection.source], selection.filename));
      },
    );
  },
  locator: () => settings.toggle("locator"),
  status: () => settings.toggle("status"),
  toolbar: () => settings.toggle("toolbar"),
  redraw: () => run({ type: "state" }),
  draw: () => run({ type: "state" }),
  close: closeDialog,
};

function closeSubmenus(menu: HTMLDetailsElement): void {
  for (const child of menu.querySelectorAll<HTMLDetailsElement>("details[open]"))
    child.open = false;
}

function closeMenus(except?: HTMLDetailsElement): void {
  for (const menu of menus) {
    if (menu === except) continue;
    closeSubmenus(menu);
    menu.open = false;
  }
}

for (const menu of menus)
  menu.addEventListener("toggle", () => {
    if (menu.open) closeMenus(menu);
    else closeSubmenus(menu);
  });
for (const submenu of document.querySelectorAll<HTMLDetailsElement>("nav .menu-nested"))
  submenu.addEventListener("toggle", () => {
    if (!submenu.open) return;
    for (const sibling of submenu.parentElement?.children ?? []) {
      if (sibling !== submenu && sibling instanceof HTMLDetailsElement) sibling.open = false;
    }
  });
for (const control of controls) {
  control.addEventListener("click", () => {
    closeMenus();
    const action = control.dataset["action"];
    if (action) void Promise.resolve(actions[action]?.()).catch(report);
  });
}
document.addEventListener("click", (event) => {
  if (event.target instanceof Element && !event.target.closest("nav")) closeMenus();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeMenus();
    plotNavigation.cancel();
  }
  if (element<HTMLDialogElement>("properties").open || !(event.metaKey || event.ctrlKey)) return;
  if (event.ctrlKey && document.activeElement === canvas && editable()) {
    const action = { a: "autoscale", z: "zoom" }[event.key.toLowerCase() as "a" | "z"];
    if (action) {
      event.preventDefault();
      void Promise.resolve(actions[action]?.()).catch(report);
      return;
    }
  }
  const keys: Record<string, string> = { o: "open", s: "save", p: "print" };
  const action = keys[event.key.toLowerCase()];
  if (!action || (action !== "open" && !editable())) return;
  event.preventDefault();
  void Promise.resolve(actions[action]?.()).catch(report);
});
window.addEventListener("beforeunload", (event) => {
  if (edited) {
    event.preventDefault();
    event.returnValue = "";
  }
});

refreshControls();
settings.apply();
if (sessionDemo) demo.start();
void run({ type: "state" }).catch(report);

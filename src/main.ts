import coolingCurve from "./examples/cooling.agr";
import measurementErrors from "./examples/measurement_errors.agr";
import { closeDialog, element, field, input, paragraph, select, showDialog } from "./web/dialog";
import { engine } from "./web/engine";
import type { Bounds, Command, Graph, Project, Request } from "./web/protocol";

const canvas = element("canvas");
const plot = element<HTMLImageElement>("plot");
const band = element("rubber-band");
const status = element("status-bar");
const graphSelect = element<HTMLSelectElement>("graph-select");
const projectFile = element<HTMLInputElement>("project-file");
const dataFile = element<HTMLInputElement>("data-file");
const controls = Array.from(document.querySelectorAll<HTMLButtonElement>("button[data-action]"));
const menus = Array.from(document.querySelectorAll<HTMLDetailsElement>("nav details"));
const editControls = controls.filter((button) => button.hasAttribute("data-edit"));
const examples = [
  { filename: "cooling.agr", title: "Cooling curve", source: coolingCurve },
  {
    filename: "measurement_errors.agr",
    title: "Calibration measurements with error bars",
    source: measurementErrors,
  },
] as const;

let project: Project | undefined;
let currentGraph = 0;
let currentSet = 0;
let projectName = "Untitled.agr";
let plotUrl = "";
let original: File | undefined;
let checkpoint: Blob | undefined;
let edited = false;
let active = false;
let zoom = false;
let drag: { x: number; y: number; id: number } | undefined;

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
    if (request.type === "apply" || request.type === "import") edited = true;
    if (!graph()) currentGraph = Number(Object.keys(project.graphs)[0] ?? 0);
    graphSelect.replaceChildren(
      ...Object.keys(project.graphs).map((id) => new Option(`G${id}`, id)),
    );
    graphSelect.value = String(currentGraph);
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

dataFile.addEventListener("change", () => {
  const file = dataFile.files?.[0];
  dataFile.value = "";
  if (file)
    void file
      .arrayBuffer()
      .then((bytes) => run({ type: "import", graph: currentGraph, bytes }))
      .catch(report);
});

graphSelect.addEventListener("change", () => {
  currentGraph = Number(graphSelect.value);
  currentSet = 0;
  writeStatus(description());
});

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
  const scale = settings[`${axis}axes scale`]?.toLowerCase() === "logarithmic" ? "log" : "normal";
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
            ["log", "Logarithmic"],
          ],
          scale,
        ),
      ),
      field("Label", input("label", unquote(settings[`${axis}axis label`] ?? ""))),
      field("Major spacing", input("major", settings[`${axis}axis tick major`] ?? "0.2", "number")),
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
        log: data.get("scale") === "log",
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

function plotPosition(clientX: number, clientY: number): [number, number] | undefined {
  const selected = graph();
  if (!selected || !project) return undefined;
  const rect = plot.getBoundingClientRect();
  const [width, height] = project.page;
  const scale = Math.min(width, height);
  const [left, bottom, right, top] = selected.view;
  const x = ((((clientX - rect.left) / rect.width) * width) / scale - left) / (right - left);
  const y = (((1 - (clientY - rect.top) / rect.height) * height) / scale - bottom) / (top - bottom);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return undefined;
  return [x, y];
}

function coordinate(selected: Graph, index: 0 | 1, fraction: number): number {
  const start = index === 0 ? selected.world[0] : selected.world[1];
  const end = index === 0 ? selected.world[2] : selected.world[3];
  const axis = index === 0 ? "x" : "y";
  const t = selected.settings[`${axis}axes invert`] === "on" ? 1 - fraction : fraction;
  if (selected.settings[`${axis}axes scale`]?.toLowerCase() === "logarithmic") {
    return Math.exp((1 - t) * Math.log(start) + t * Math.log(end));
  }
  return (1 - t) * start + t * end;
}

function changeView(x0: number, y0: number, x1: number, y1: number): void {
  const selected = graph();
  if (!selected || !editable()) return;
  const xa = coordinate(selected, 0, x0);
  const xb = coordinate(selected, 0, x1);
  const ya = coordinate(selected, 1, y0);
  const yb = coordinate(selected, 1, y1);
  const bounds: Bounds = [Math.min(xa, xb), Math.min(ya, yb), Math.max(xa, xb), Math.max(ya, yb)];
  void apply({ type: "world", graph: currentGraph, bounds }).catch(report);
}

function setZoom(enabled: boolean): void {
  zoom = enabled;
  drag = undefined;
  band.hidden = true;
  canvas.classList.toggle("zoom", enabled);
  controls
    .find((button) => button.dataset["action"] === "zoom")
    ?.setAttribute("aria-pressed", String(enabled));
}

canvas.addEventListener("pointermove", (event) => {
  const pos = plotPosition(event.clientX, event.clientY);
  const selected = graph();
  if (pos && selected) {
    const [x, y] = pos;
    element("locator").textContent =
      `G${currentGraph}: X = ${coordinate(selected, 0, x).toPrecision(6)}   Y = ${coordinate(selected, 1, y).toPrecision(6)}`;
  }
  if (!drag || drag.id !== event.pointerId) return;
  const rect = canvas.getBoundingClientRect();
  band.hidden = false;
  band.style.left = `${Math.min(drag.x, event.clientX) - rect.left}px`;
  band.style.top = `${Math.min(drag.y, event.clientY) - rect.top}px`;
  band.style.width = `${Math.abs(event.clientX - drag.x)}px`;
  band.style.height = `${Math.abs(event.clientY - drag.y)}px`;
});
canvas.addEventListener("pointerdown", (event) => {
  if (!zoom || event.button !== 0) return;
  drag = { x: event.clientX, y: event.clientY, id: event.pointerId };
  canvas.setPointerCapture(event.pointerId);
});
canvas.addEventListener("pointerup", (event) => {
  if (!drag || drag.id !== event.pointerId) return;
  const start = plotPosition(drag.x, drag.y);
  const end = plotPosition(event.clientX, event.clientY);
  setZoom(false);
  if (start && end && Math.abs(start[0] - end[0]) > 0.002 && Math.abs(start[1] - end[1]) > 0.002) {
    changeView(...start, ...end);
  }
});
canvas.addEventListener("pointercancel", () => setZoom(false));

const actions: Record<string, () => void | Promise<void>> = {
  new: () =>
    confirmDiscard(async () => {
      await run({ type: "new" });
      original = undefined;
      checkpoint = undefined;
      edited = false;
      projectName = "Untitled.agr";
      refreshControls();
    }),
  open: () => confirmDiscard(() => projectFile.click()),
  revert: () =>
    confirmDiscard(async () => {
      if (!checkpoint) return;
      await run({ type: "open", bytes: await checkpoint.arrayBuffer() });
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
    showDialog(
      "Data sets",
      Object.entries(graph()?.sets ?? {}).map(([id, set]) =>
        paragraph(`G${currentGraph}.S${id}: ${set.kind.toUpperCase()}, ${set.count} points`),
      ),
    ),
  graphs: () =>
    showDialog(
      "Graphs",
      Object.keys(project?.graphs ?? {}).map((id) => paragraph(`G${id}`)),
    ),
  diagnostics: () =>
    showDialog(
      "Console",
      project?.diagnostics.length
        ? project.diagnostics.map(({ line, message }) =>
            paragraph(`Line ${line ?? "?"}: ${message}`),
          )
        : [paragraph(status.textContent ?? "")],
    ),
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
  locator: () => {
    const bar = element("locator-bar");
    bar.hidden = !bar.hidden;
  },
  status: () => {
    status.hidden = !status.hidden;
  },
  toolbar: () => {
    const bar = element("toolbar");
    bar.hidden = !bar.hidden;
  },
  redraw: () => run({ type: "state" }),
  draw: () => run({ type: "state" }),
  zoom: () => setZoom(!zoom),
  "zoom-in": () => changeView(0.05, 0.05, 0.95, 0.95),
  "zoom-out": () => changeView(-0.05, -0.05, 1.05, 1.05),
  left: () => changeView(-0.05, 0, 0.95, 1),
  right: () => changeView(0.05, 0, 1.05, 1),
  up: () => changeView(0, 0.05, 1, 1.05),
  down: () => changeView(0, -0.05, 1, 0.95),
  close: closeDialog,
};

function closeMenus(except?: HTMLDetailsElement): void {
  for (const menu of menus) if (menu !== except) menu.open = false;
}

for (const menu of menus)
  menu.addEventListener("toggle", () => {
    if (menu.open) closeMenus(menu);
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
    setZoom(false);
  }
  if (element<HTMLDialogElement>("properties").open || !(event.metaKey || event.ctrlKey)) return;
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
void run({ type: "state" }).catch(report);

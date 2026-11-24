import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import { guidedDemoHint } from "./guided_demo";
import { isSetKind, setColumns } from "./set_types";
import type { Command, Graph, Region, SeriesOperation, SetKind } from "./protocol";

type Apply = (command: Command) => Promise<void>;

export type DataContext = {
  graph: number;
  currentSet: number;
  selected: () => Graph | undefined;
  apply: Apply;
  regions: Record<string, Region>;
};

const pairs = (values: readonly string[]): readonly (readonly [string, string])[] =>
  values.map((value) => [value, value]);

function integer(name: string, value: number, minimum = 1): HTMLInputElement {
  const control = input(name, String(value), "number");
  control.min = String(minimum);
  control.step = "1";
  return control;
}

function checkbox(name: string, label: string, checked = false): HTMLLabelElement {
  const control = document.createElement("input");
  control.name = name;
  control.type = "checkbox";
  control.checked = checked;
  const wrapper = document.createElement("label");
  wrapper.className = "check-field";
  wrapper.append(control, label);
  return wrapper;
}

function sets(context: DataContext): readonly (readonly [string, string])[] {
  return Object.entries(context.selected()?.sets ?? {})
    .filter(([, set]) => set.count > 0)
    .map(([id, set]) => [id, `G${context.graph}.S${id}  (${set.count} points)`]);
}

function setPicker(context: DataContext, name = "set"): HTMLSelectElement | undefined {
  const options = sets(context);
  if (!options.length) return undefined;
  const current = options.some(([id]) => Number(id) === context.currentSet)
    ? String(context.currentSet)
    : options[0]![0];
  return select(name, options, current);
}

function requireSet(context: DataContext, title: string): HTMLSelectElement | undefined {
  const picker = setPicker(context);
  if (!picker) showDialog(title, [paragraph("Create or import an XY data set first.")]);
  return picker;
}

function selectedSet(data: FormData): number {
  return Number(data.get("set"));
}

export function formulaDialog(context: DataContext): void {
  const hint = guidedDemoHint("formula");
  const guide = hint ? paragraph(hint) : undefined;
  if (guide) guide.className = "dialog-guide-hint";
  showDialog(
    "Create by formula",
    [
      ...(guide ? [guide] : []),
      paragraph("Use $t as the sampled parameter. Functions include sin, cos, exp, log, and sqrt."),
      field("Start", input("start", "0")),
      field("Stop", input("stop", "2*pi")),
      field("Points", integer("points", 100, 2)),
      field("X", input("x", "$t")),
      field("Y", input("y", "sin($t)")),
    ],
    (data) =>
      context.apply({
        type: "generate",
        graph: context.graph,
        start: formText(data, "start"),
        stop: formText(data, "stop"),
        points: Number(data.get("points")),
        x: formText(data, "x"),
        y: formText(data, "y"),
      }),
  );
}

function dataRows(source: string, columns: number): number[][] {
  const rows: number[][] = [];
  for (const [index, line] of source.split(/\r?\n/).entries()) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const row = trimmed.split(/[\s,]+/).map(Number);
    if (row.length !== columns)
      throw new Error(`Line ${index + 1} needs exactly ${columns} numeric columns.`);
    if (!row.every(Number.isFinite))
      throw new Error(`Line ${index + 1} contains a non-finite number.`);
    rows.push(row);
  }
  if (!rows.length) throw new Error("Enter at least one data row.");
  return rows;
}

/** Opens the browser equivalent of Grace's Create new > In text editor action. */
export function textEditorDialog(context: DataContext): void {
  const kinds = Object.keys(setColumns) as SetKind[];
  const kind = select(
    "kind",
    kinds.map((candidate) => [candidate, candidate.toUpperCase()]),
    "xy",
  );
  const source = document.createElement("textarea");
  source.name = "data";
  source.rows = 12;
  source.spellcheck = false;
  source.placeholder = "0 1\n1 2\n2 3";
  source.setAttribute("aria-label", "Data rows");
  const sourceField = document.createElement("label");
  sourceField.append("Data rows", source);
  showDialog(
    "Create data set in text editor",
    [
      paragraph(
        "Enter one row per line. Separate finite numeric columns with spaces or commas. Lines beginning with # are comments.",
      ),
      field("Set type", kind),
      sourceField,
    ],
    (form) => {
      const selected = formText(form, "kind");
      if (!isSetKind(selected)) throw new Error("Choose a supported set type.");
      return context.apply({
        type: "replaceData",
        graph: context.graph,
        set: null,
        kind: selected,
        rows: dataRows(formText(form, "data"), setColumns[selected].length),
      });
    },
  );
}

export function evaluateDialog(context: DataContext): void {
  const picker = requireSet(context, "Evaluate expression");
  if (!picker) return;
  showDialog(
    "Evaluate expression",
    [
      field("Source set", picker),
      paragraph(
        "Blank expressions preserve the coordinate. Use x, y, sin(x), or set references such as s1.y.",
      ),
      field("X expression", input("x", "")),
      field("Y expression", input("y", "")),
      field(
        "Restriction",
        select(
          "region",
          [
            ["", "None"],
            ...Object.entries(context.regions)
              .filter(
                ([, region]) =>
                  region.active !== false &&
                  (region.graph === null || region.graph === context.graph),
              )
              .map(([id, region]): [string, string] => [id, `R${id} ${region.kind}`]),
          ],
          "",
        ),
      ),
      checkbox("negate", "Negate restriction"),
      checkbox("replace", "Replace source set"),
    ],
    (data) =>
      context.apply({
        type: "evaluate",
        graph: context.graph,
        set: selectedSet(data),
        x: formText(data, "x"),
        y: formText(data, "y"),
        ...(data.get("region") ? { region: Number(data.get("region")) } : {}),
        negate: data.has("negate"),
        replace: data.has("replace"),
      }),
  );
}

export function fourierDialog(context: DataContext): void {
  const picker = requireSet(context, "Fourier transforms");
  if (!picker) return;
  const hint = guidedDemoHint("fourier");
  const guide = hint ? paragraph(hint) : undefined;
  if (guide) guide.className = "dialog-guide-hint";
  showDialog(
    "Fourier transforms",
    [
      ...(guide ? [guide] : []),
      field("Input set", picker),
      field(
        "Algorithm",
        select(
          "algorithm",
          [
            ["fft", "Fast Fourier transform"],
            ["dft", "Direct Fourier transform"],
          ],
          "fft",
        ),
      ),
      field(
        "Input data",
        select(
          "input",
          [
            ["real", "Real Y data"],
            ["complex", "Complex X + iY data"],
          ],
          "real",
        ),
      ),
      field(
        "Direction",
        select(
          "direction",
          [
            ["forward", "Forward"],
            ["inverse", "Inverse"],
          ],
          "forward",
        ),
      ),
      field(
        "Load",
        select(
          "output",
          [
            ["magnitude", "Magnitude"],
            ["phase", "Phase"],
            ["coefficients", "Coefficients"],
            ["windowed", "Windowed data"],
          ],
          "magnitude",
        ),
      ),
      field(
        "Abscissa",
        select(
          "abscissa",
          [
            ["index", "Index"],
            ["frequency", "Frequency"],
            ["period", "Period"],
          ],
          "index",
        ),
      ),
      field(
        "Window",
        select(
          "window",
          [
            ["none", "None"],
            ["triangular", "Triangular"],
            ["hann", "Hanning"],
            ["welch", "Welch"],
            ["hamming", "Hamming"],
            ["blackman", "Blackman"],
            ["parzen", "Parzen"],
          ],
          "none",
        ),
      ),
    ],
    (data) =>
      context.apply({
        type: "fourier",
        graph: context.graph,
        set: selectedSet(data),
        options: {
          algorithm: formText(data, "algorithm") as "fft" | "dft",
          input: formText(data, "input") as "real" | "complex",
          direction: formText(data, "direction") as "forward" | "inverse",
          output: formText(data, "output") as "magnitude" | "phase" | "coefficients" | "windowed",
          abscissa: formText(data, "abscissa") as "index" | "frequency" | "period",
          window: formText(data, "window") as
            "none" | "triangular" | "hann" | "welch" | "hamming" | "blackman" | "parzen",
        },
      }),
  );
}

export function histogramDialog(context: DataContext): void {
  const picker = requireSet(context, "Histograms");
  if (!picker) return;
  showDialog(
    "Histograms",
    [
      field("Input set", picker),
      field("Start", input("start", "0", "number")),
      field("Stop", input("stop", "1", "number")),
      field("Bins", integer("bins", 20, 1)),
      checkbox("cumulative", "Cumulative"),
      checkbox("normalize", "Normalize to one"),
    ],
    (data) =>
      context.apply({
        type: "series",
        graph: context.graph,
        set: selectedSet(data),
        operation: {
          method: "histogram",
          start: Number(data.get("start")),
          stop: Number(data.get("stop")),
          bins: Number(data.get("bins")),
          cumulative: data.has("cumulative"),
          normalize: data.has("normalize"),
        },
      }),
  );
}

export function runningDialog(context: DataContext): void {
  const picker = requireSet(context, "Running averages");
  if (!picker) return;
  showDialog(
    "Running averages",
    [
      field("Input set", picker),
      field("Window length", integer("length", 5, 1)),
      field(
        "Statistic",
        select(
          "statistic",
          [
            ["mean", "Mean"],
            ["median", "Median"],
            ["minimum", "Minimum"],
            ["maximum", "Maximum"],
            ["stddev", "Standard deviation"],
          ],
          "mean",
        ),
      ),
    ],
    (data) =>
      context.apply({
        type: "series",
        graph: context.graph,
        set: selectedSet(data),
        operation: {
          method: "running",
          length: Number(data.get("length")),
          statistic: formText(data, "statistic") as
            "mean" | "median" | "minimum" | "maximum" | "stddev",
        },
      }),
  );
}

export function interpolationDialog(context: DataContext): void {
  const picker = requireSet(context, "Interpolation and splines");
  if (!picker) return;
  showDialog(
    "Interpolation and splines",
    [
      field("Input set", picker),
      field("Start", input("start", "0", "number")),
      field("Stop", input("stop", "1", "number")),
      field("Points", integer("points", 100, 2)),
      field(
        "Method",
        select(
          "interpolation",
          [
            ["linear", "Linear"],
            ["spline", "Spline"],
            ["akima", "Akima spline"],
          ],
          "linear",
        ),
      ),
      checkbox("strict", "Reject points beyond the input range"),
    ],
    (data) =>
      context.apply({
        type: "series",
        graph: context.graph,
        set: selectedSet(data),
        operation: {
          method: "interpolate",
          start: Number(data.get("start")),
          stop: Number(data.get("stop")),
          points: Number(data.get("points")),
          interpolation: formText(data, "interpolation") as "linear" | "spline" | "akima",
          strict: data.has("strict"),
        },
      }),
  );
}

export function regressionDialog(context: DataContext): void {
  const picker = requireSet(context, "Regression");
  if (!picker) return;
  showDialog(
    "Regression",
    [
      field("Input set", picker),
      field(
        "Model",
        select(
          "model",
          [
            ["polynomial", "Polynomial"],
            ["power", "Power"],
            ["exponential", "Exponential"],
            ["logarithmic", "Logarithmic"],
            ["reciprocal", "Reciprocal"],
          ],
          "polynomial",
        ),
      ),
      field("Polynomial degree", integer("degree", 1, 0)),
      field(
        "Output",
        select(
          "output",
          [
            ["fitted", "Fitted values"],
            ["residuals", "Residuals"],
          ],
          "fitted",
        ),
      ),
    ],
    (data) =>
      context.apply({
        type: "series",
        graph: context.graph,
        set: selectedSet(data),
        operation: {
          method: "regression",
          degree: formText(data, "model") === "polynomial" ? Number(data.get("degree")) : 1,
          model: formText(data, "model") as NonNullable<
            Extract<SeriesOperation, { method: "regression" }>["model"]
          >,
          output: data.get("output") === "residuals" ? "residuals" : "fitted",
        },
      }),
  );
}

export function differenceDialog(context: DataContext, seasonal = false): void {
  const title = seasonal ? "Seasonal differences" : "Differences";
  const picker = requireSet(context, title);
  if (!picker) return;
  const controls: HTMLElement[] = [field("Input set", picker)];
  if (seasonal) controls.push(field("Period", integer("period", 1, 1)));
  else
    controls.push(
      field("Scheme", select("scheme", pairs(["forward", "backward", "centered"]), "forward")),
    );
  showDialog(title, controls, (data) =>
    context.apply({
      type: "series",
      graph: context.graph,
      set: selectedSet(data),
      operation: seasonal
        ? { method: "seasonalDifference", period: Number(data.get("period")) }
        : {
            method: "difference",
            scheme: formText(data, "scheme") as "forward" | "backward" | "centered",
          },
    }),
  );
}

export function integrationDialog(context: DataContext): void {
  const picker = requireSet(context, "Integration");
  if (!picker) return;
  showDialog(
    "Integration",
    [
      field("Input set", picker),
      paragraph("Creates a new set using cumulative trapezoidal integration."),
    ],
    (data) =>
      context.apply({
        type: "series",
        graph: context.graph,
        set: selectedSet(data),
        operation: { method: "integrate" },
      }),
  );
}

type OperationScope = "points" | "sets";
type PointMethod = "sort" | "reverse" | "join" | "split" | "drop";
type SetMethod = "copy" | "move" | "swap" | "delete" | "duplicate" | "clear";

const pointMethods: readonly (readonly [PointMethod, string])[] = [
  ["sort", "Sort points"],
  ["reverse", "Reverse points"],
  ["join", "Join sets"],
  ["split", "Split set"],
  ["drop", "Drop point range"],
];
const setMethods: readonly (readonly [SetMethod, string])[] = [
  ["copy", "Copy set"],
  ["move", "Move set"],
  ["swap", "Swap sets"],
  ["delete", "Delete set"],
  ["duplicate", "Duplicate set"],
  ["clear", "Clear data"],
];

function operationDialog(
  context: DataContext,
  scope: OperationScope,
  method: PointMethod | SetMethod,
): void {
  const title = scope === "points" ? "Data set operations" : "Set operations";
  const picker = requireSet(context, title);
  if (!picker) return;
  const methods = scope === "points" ? pointMethods : setMethods;
  const methodPicker = select("method", methods, method);
  methodPicker.addEventListener("change", () =>
    operationDialog(context, scope, methodPicker.value as PointMethod | SetMethod),
  );
  const controls: HTMLElement[] = [field("Operation", methodPicker), field("Set", picker)];
  if (method === "sort") {
    controls.push(
      field(
        "Sort by",
        select(
          "axis",
          [
            ["x", "X"],
            ["y", "Y"],
            ["y1", "Y1"],
            ["y2", "Y2"],
            ["y3", "Y3"],
            ["y4", "Y4"],
          ],
          "x",
        ),
      ),
      checkbox("descending", "Descending sort"),
    );
  }
  if (method === "join")
    controls.push(
      field(
        "Other set",
        setPicker(context, "other") ?? select("other", [["0", "No other set"]], "0"),
      ),
    );
  if (method === "split") controls.push(field("Length", integer("length", 10, 1)));
  if (method === "drop")
    controls.push(
      field("First point", integer("first", 0, 0)),
      field("Last point", integer("last", 0, 0)),
      paragraph("Point ranges are zero-based and inclusive."),
    );
  if (method === "copy" || method === "move" || method === "swap")
    controls.push(
      field("Target graph", integer("target-graph", context.graph, 0)),
      field("Target set", input("target-set", "")),
    );
  showDialog(title, controls, (data) => {
    const set = selectedSet(data);
    const operation: Extract<Command, { type: "setOperation" }>["operation"] =
      method === "sort"
        ? {
            method,
            set,
            axis: formText(data, "axis") as "x" | "y" | "y1" | "y2" | "y3" | "y4",
            descending: data.has("descending"),
          }
        : method === "reverse" ||
            method === "delete" ||
            method === "duplicate" ||
            method === "clear"
          ? { method, set }
          : method === "join"
            ? { method, set, other: Number(data.get("other")) }
            : method === "split"
              ? { method, set, length: Number(data.get("length")) }
              : method === "drop"
                ? {
                    method,
                    set,
                    first: Number(data.get("first")),
                    last: Number(data.get("last")),
                  }
                : {
                    method,
                    set,
                    targetGraph: Number(data.get("target-graph")),
                    targetSet: formText(data, "target-set").trim()
                      ? Number(data.get("target-set"))
                      : undefined,
                  };
    return context.apply({ type: "setOperation", graph: context.graph, operation });
  });
}

export function setOperationsDialog(context: DataContext, scope: OperationScope = "points"): void {
  operationDialog(context, scope, scope === "points" ? "sort" : "copy");
}

export function dataSetsDialog(
  context: DataContext,
  create: () => void,
  spreadsheet: ((set: number) => void) | undefined,
  newSpreadsheet: () => void,
  importBlockData?: () => void,
): void {
  const picker = setPicker(context);
  const rows = Object.entries(context.selected()?.sets ?? {}).map(([id, set]) =>
    paragraph(`G${context.graph}.S${id}: ${set.kind.toUpperCase()}, ${set.count} points`),
  );
  showDialog(
    "Data sets",
    rows.length
      ? [
          field("Select set", picker ?? select("set", [["0", "No data sets"]], "0")),
          ...rows,
          paragraph("Use Data set operations to reorder, combine, split, or prune points."),
        ]
      : [paragraph("This graph has no data sets yet.")],
  );
  const createMenu = document.createElement("details");
  createMenu.className = "dialog-submenu";
  const createSummary = document.createElement("summary");
  createSummary.textContent = "Create new";
  const createButton = document.createElement("button");
  createButton.type = "button";
  createButton.textContent = "By formula...";
  createButton.addEventListener("click", create);
  const createTableButton = document.createElement("button");
  createTableButton.type = "button";
  createTableButton.textContent = "In spreadsheet...";
  createTableButton.addEventListener("click", newSpreadsheet);
  const createTextButton = document.createElement("button");
  createTextButton.type = "button";
  createTextButton.textContent = "In text editor...";
  createTextButton.addEventListener("click", () => textEditorDialog(context));
  createMenu.append(createSummary, createTableButton, createTextButton, createButton);
  if (importBlockData) {
    const blockButton = document.createElement("button");
    blockButton.type = "button";
    blockButton.textContent = "From block data...";
    blockButton.addEventListener("click", importBlockData);
    createMenu.append(blockButton);
  }
  document.getElementById("dialog-content")?.append(createMenu);
  if (spreadsheet) {
    const editButton = document.createElement("button");
    editButton.type = "button";
    editButton.textContent = "Edit data...";
    editButton.addEventListener("click", () =>
      spreadsheet(Number(picker?.value ?? context.currentSet)),
    );
    document.getElementById("dialog-content")?.append(editButton);
  }
}

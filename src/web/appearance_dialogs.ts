import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import type { Command, Graph } from "./protocol";

type Context = {
  graph: () => Graph | undefined;
  activeGraph: () => number;
  activeSet: () => number;
  apply: (command: Command) => Promise<void>;
};

type AppearanceOperation = Extract<Command, { type: "appearance" }>["operation"];

function number(data: FormData, name: string, minimum = 0): number {
  const value = Number(data.get(name));
  if (!Number.isFinite(value) || value < minimum)
    throw new Error(`Enter a finite ${name} of at least ${minimum}.`);
  return value;
}

function integer(data: FormData, name: string, minimum = 0): number {
  const value = number(data, name, minimum);
  if (!Number.isInteger(value)) throw new Error(`Enter a whole-number ${name}.`);
  return value;
}

function boundedInteger(data: FormData, name: string, maximum: number): number {
  const value = integer(data, name);
  if (value > maximum) throw new Error(`Enter a ${name} no greater than ${maximum}.`);
  return value;
}

function positive(data: FormData, name: string): number {
  const value = number(data, name);
  if (value <= 0) throw new Error(`Enter a ${name} greater than zero.`);
  return value;
}

function enabled(value: string | undefined, fallback: boolean): boolean {
  if (!value) return fallback;
  return value === "on" || value === "true";
}

function checkbox(name: string, label: string, checked: boolean): HTMLLabelElement {
  const control = document.createElement("input");
  control.name = name;
  control.type = "checkbox";
  control.checked = checked;
  const wrapper = document.createElement("label");
  wrapper.className = "check-field";
  wrapper.append(control, label);
  return wrapper;
}

function apply(context: Context, operation: AppearanceOperation): Promise<void> {
  return context.apply({ type: "appearance", operation });
}

function requireGraph(context: Context, title: string): Graph | undefined {
  const graph = context.graph();
  if (!graph) showDialog(title, [paragraph("Open or create a Grace project first.")]);
  return graph;
}

function graphDialog(context: Context): void {
  const graph = requireGraph(context, "Graph rendering");
  if (!graph) return;
  const settings = graph.settings;
  showDialog(
    "Graph rendering",
    [
      field(
        "Graph type",
        select(
          "kind",
          [
            ["XY", "XY"],
            ["Chart", "XY chart"],
            ["Polar", "Polar"],
            ["Pie", "Pie"],
            ["Fixed", "Fixed"],
          ],
          ["XY", "Chart", "Polar", "Pie", "Fixed"].find(
            (kind) => kind.toLowerCase() === graph.kind.toLowerCase(),
          ) ?? "XY",
        ),
      ),
      checkbox("legend", "Show legend", enabled(settings["legend"], true)),
      checkbox("frame", "Show frame", Number(settings["frame pattern"] ?? "1") !== 0),
      field("Frame width", input("frameWidth", settings["frame linewidth"] ?? "1", "number")),
      field("Title size", input("titleSize", settings["title size"] ?? "1.5", "number")),
      field("Subtitle size", input("subtitleSize", settings["subtitle size"] ?? "1", "number")),
      checkbox("stacked", "Stack chart bars", enabled(settings["stacked"], false)),
      field("Chart bar gap", input("barGap", settings["bar hgap"] ?? "0", "number")),
    ],
    (data) => {
      const barGap = number(data, "barGap");
      if (barGap > 0.9) throw new Error("Chart bar gap must be at most 0.9.");
      return apply(context, {
        method: "graph",
        graph: context.activeGraph(),
        kind: formText(data, "kind"),
        legend: data.has("legend"),
        frame: data.has("frame"),
        frameWidth: positive(data, "frameWidth"),
        titleSize: positive(data, "titleSize"),
        subtitleSize: positive(data, "subtitleSize"),
        stacked: data.has("stacked"),
        barGap,
      });
    },
  );
}

function setDialog(context: Context): void {
  const graph = requireGraph(context, "Set rendering");
  const set = graph?.sets[String(context.activeSet())];
  if (!graph || !set) {
    if (graph) showDialog("Set rendering", [paragraph("Select an existing data set first.")]);
    return;
  }
  const settings = set.settings;
  showDialog(
    "Set rendering",
    [
      field("Line width", input("lineWidth", settings["line linewidth"] ?? "1", "number")),
      field(
        "Line style",
        select(
          "lineStyle",
          [
            ["0", "No line"],
            ["1", "Solid"],
            ["2", "Dotted"],
            ["3", "Dashed"],
            ["4", "Dot-dashed"],
            ["5", "Long dashed"],
            ["6", "Long dash-dot"],
            ["7", "Long dash-double dot"],
            ["8", "Long dash-triple dot"],
          ],
          settings["line linestyle"] ?? "1",
        ),
      ),
      field("Symbol size", input("symbolSize", settings["symbol size"] ?? "1", "number")),
      field("Draw every symbol", input("symbolSkip", settings["symbol skip"] ?? "0", "number")),
      checkbox("errorBars", "Show error bars", enabled(settings["errorbar"], true)),
      field("Error-bar size", input("errorSize", settings["errorbar size"] ?? "1", "number")),
      field(
        "Fill type",
        select(
          "fillType",
          [
            ["0", "None"],
            ["1", "Polygon"],
            ["2", "To baseline"],
          ],
          settings["fill type"] ?? "0",
        ),
      ),
      field("Fill color", input("fillColor", settings["fill color"] ?? "1", "number")),
      field(
        "Baseline type",
        select(
          "baselineType",
          [
            ["0", "Zero"],
            ["1", "Set minimum"],
            ["2", "Set maximum"],
            ["3", "Graph minimum"],
            ["4", "Graph maximum"],
            ["5", "Set average"],
          ],
          settings["baseline type"] ?? "0",
        ),
      ),
      checkbox("baseline", "Draw baseline", enabled(settings["baseline"], false)),
      checkbox("dropLines", "Draw drop lines", enabled(settings["dropline"], false)),
    ],
    (data) =>
      apply(context, {
        method: "set",
        graph: context.activeGraph(),
        set: context.activeSet(),
        lineWidth: positive(data, "lineWidth"),
        lineStyle: integer(data, "lineStyle"),
        symbolSize: positive(data, "symbolSize"),
        symbolSkip: integer(data, "symbolSkip"),
        errorBars: data.has("errorBars"),
        errorSize: positive(data, "errorSize"),
        fillType: integer(data, "fillType"),
        fillColor: integer(data, "fillColor"),
        baselineType: integer(data, "baselineType"),
        baseline: data.has("baseline"),
        dropLines: data.has("dropLines"),
      }),
  );
}

const axisLabels = {
  x: "X axis",
  y: "Y axis",
  alternateX: "Alternate X axis",
  alternateY: "Alternate Y axis",
} as const;
type Axis = keyof typeof axisLabels;

const tickFormats: [string, string][] = [
  ["general", "General"],
  ["decimal", "Decimal"],
  ["exponential", "Exponential"],
  ["power", "Power"],
  ["scientific", "Scientific"],
  ["engineering", "Engineering"],
  ["computing", "Computing"],
  ["ddmmyy", "DD-MM-YY"],
  ["mmddyy", "MM-DD-YY"],
  ["yymmdd", "YY-MM-DD"],
  ["mmyy", "MM-YY"],
  ["mmdd", "MM-DD"],
  ["monthday", "Month and day"],
  ["daymonth", "Day and month"],
  ["months", "Abbreviated month"],
  ["monthsy", "Abbreviated month and year"],
  ["monthl", "Month"],
  ["dayofweeks", "Abbreviated day of week"],
  ["dayofweekl", "Day of week"],
  ["dayofyear", "Day of year"],
  ["hms", "HH:MM:SS"],
  ["mmddhms", "MM-DD HH:MM:SS"],
  ["mmddyyhms", "MM-DD-YY HH:MM:SS"],
  ["yymmddhms", "YY-MM-DD HH:MM:SS"],
  ["degreeslon", "Degrees longitude"],
  ["degreesmmlon", "Degrees and minutes longitude"],
  ["degreesmmsslon", "Degrees, minutes and seconds longitude"],
  ["mmsslon", "Minutes and seconds longitude"],
  ["degreeslat", "Degrees latitude"],
  ["degreesmmlat", "Degrees and minutes latitude"],
  ["degreesmmsslat", "Degrees, minutes and seconds latitude"],
  ["mmsslat", "Minutes and seconds latitude"],
];

function axisDialog(context: Context, axis: Axis): void {
  const graph = requireGraph(context, "Axis rendering");
  if (!graph) return;
  const settings = graph.settings;
  const primary = axis === "x" || axis === "y";
  const prefix = primary ? `${axis}axis` : axis === "alternateX" ? "altxaxis" : "altyaxis";
  const chooser = select("axis", Object.entries(axisLabels), axis);
  chooser.addEventListener("change", () => axisDialog(context, chooser.value as Axis));
  showDialog(
    "Axis rendering",
    [
      field("Edit", chooser),
      checkbox("visible", "Display axis", enabled(settings[prefix], primary)),
      ...(primary
        ? [
            checkbox("invert", "Invert axis", enabled(settings[`${axis}axes invert`], false)),
            checkbox(
              "grid",
              "Show major grid",
              enabled(settings[`${prefix} tick major grid`], false),
            ),
            checkbox(
              "minorGrid",
              "Show minor grid",
              enabled(settings[`${prefix} tick minor grid`], false),
            ),
          ]
        : [
            field(
              "Label",
              input("label", (settings[`${prefix} label`] ?? "").replace(/^"|"$/g, "")),
            ),
          ]),
      field(
        "Minor ticks",
        input("minorTicks", settings[`${prefix} tick minor ticks`] ?? "1", "number"),
      ),
      field(
        "Label precision",
        input("precision", settings[`${prefix} ticklabel prec`] ?? "3", "number"),
      ),
      field(
        "Number format",
        select(
          "format",
          tickFormats,
          settings[`${prefix} ticklabel format`]?.toLowerCase() ?? "general",
        ),
      ),
      field(
        "Label placement",
        select(
          "placement",
          [
            ["normal", "Normal"],
            ["opposite", "Opposite"],
            ["both", "Both"],
          ],
          settings[`${prefix} ticklabel place`] ?? "normal",
        ),
      ),
      field(
        "Tick direction",
        select(
          "direction",
          [
            ["in", "In"],
            ["out", "Out"],
            ["both", "Both"],
          ],
          settings[`${prefix} tick direction`] ?? "in",
        ),
      ),
      field(
        "Stagger labels",
        input("stagger", settings[`${prefix} ticklabel stagger`] ?? "0", "number"),
      ),
    ],
    (data) =>
      apply(context, {
        method: "axis",
        graph: context.activeGraph(),
        axis,
        visible: data.has("visible"),
        ...(primary
          ? { invert: data.has("invert"), grid: data.has("grid"), minorGrid: data.has("minorGrid") }
          : { label: formText(data, "label") }),
        minorTicks: boundedInteger(data, "minorTicks", 99),
        precision: boundedInteger(data, "precision", 12),
        format: formText(data, "format"),
        placement: formText(data, "placement"),
        direction: formText(data, "direction"),
        stagger: boundedInteger(data, "stagger", 10),
      }),
  );
}

export function appearanceDialogs(context: Context): {
  graph: () => void;
  set: () => void;
  axis: (axis: "x" | "y") => void;
} {
  return {
    graph: () => graphDialog(context),
    set: () => setDialog(context),
    axis: (axis) => axisDialog(context, axis),
  };
}

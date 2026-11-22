import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import { engine } from "./engine";
import type { Command, Project, Region } from "./protocol";

type Context = {
  project: () => Project | undefined;
  activeGraph: () => number;
  apply: (command: Command) => Promise<void>;
};

const kinds = [
  ["insidePolygon", "Inside polygon"],
  ["outsidePolygon", "Outside polygon"],
  ["aboveLine", "Above line"],
  ["belowLine", "Below line"],
  ["leftOfLine", "Left of line"],
  ["rightOfLine", "Right of line"],
  ["inHorizontalRange", "In horizontal range"],
  ["outHorizontalRange", "Out of horizontal range"],
  ["inVerticalRange", "In vertical range"],
  ["outVerticalRange", "Out of vertical range"],
] as const;

type Kind = (typeof kinds)[number][0];
type PolygonKind = "insidePolygon" | "outsidePolygon";
type LineKind = "aboveLine" | "belowLine" | "leftOfLine" | "rightOfLine";
type RangeKind =
  "inHorizontalRange" | "outHorizontalRange" | "inVerticalRange" | "outVerticalRange";

function isKind(value: string): value is Kind {
  return kinds.some(([kind]) => kind === value);
}

function isPolygonKind(kind: Kind): kind is PolygonKind {
  return kind === "insidePolygon" || kind === "outsidePolygon";
}

function isLineKind(kind: Kind): kind is LineKind {
  return (
    kind === "aboveLine" || kind === "belowLine" || kind === "leftOfLine" || kind === "rightOfLine"
  );
}

function finite(data: FormData, name: string): number {
  const value = Number(data.get(name));
  if (!Number.isFinite(value)) throw new Error("Enter a finite " + name + ".");
  return value;
}

function integer(data: FormData, name: string): number {
  const value = finite(data, name);
  if (!Number.isInteger(value) || value < 0) throw new Error("Enter a non-negative " + name + ".");
  return value;
}

function regionChoices(project: Project): readonly (readonly [string, string])[] {
  return Object.entries(project.regions).map(([id, region]) => [
    id,
    "Region " + id + "  " + region.kind + (region.active ? "" : " (inactive)"),
  ]);
}

function graphChoices(project: Project): readonly (readonly [string, string])[] {
  return [
    ["all", "All graphs"],
    ...Object.keys(project.graphs).map((id) => [id, "G" + id] as const),
  ];
}

function parsePoints(value: string): [number, number][] {
  const points = value
    .trim()
    .split(/[;\n]+/)
    .filter(Boolean)
    .map((pair) =>
      pair
        .trim()
        .split(/[\s,]+/)
        .map(Number),
    );
  if (
    points.length < 3 ||
    points.some((point) => point.length !== 2 || point.some((v) => !Number.isFinite(v)))
  )
    throw new Error("Enter at least three finite X, Y pairs separated by semicolons.");
  return points as [number, number][];
}

function region(data: FormData): Region {
  const candidate = formText(data, "kind");
  if (!isKind(candidate)) throw new Error("Select a region type.");
  const kind = candidate;
  const graph = formText(data, "graph") === "all" ? null : integer(data, "graph");
  const active = data.has("active");
  if (isPolygonKind(kind))
    return { kind, points: parsePoints(formText(data, "points")), graph, active };
  if (isLineKind(kind))
    return {
      kind,
      first: [finite(data, "firstX"), finite(data, "firstY")],
      second: [finite(data, "secondX"), finite(data, "secondY")],
      graph,
      active,
    };
  const start = finite(data, "start");
  const stop = finite(data, "stop");
  if (start >= stop) throw new Error("Range start must be less than stop.");
  const rangeKind: RangeKind = kind;
  return { kind: rangeKind, start, stop, graph, active };
}

function checked(name: string, label: string, value: boolean): HTMLLabelElement {
  const control = document.createElement("input");
  control.name = name;
  control.type = "checkbox";
  control.checked = value;
  const wrapper = document.createElement("label");
  wrapper.className = "check-field";
  wrapper.append(control, label);
  return wrapper;
}

function regionStatus(context: Context): void {
  const project = context.project();
  if (!project) return;
  const regions = Object.entries(project.regions);
  showDialog(
    "Region status",
    regions.length
      ? regions.map(([id, item]) =>
          paragraph(
            "Region " +
              id +
              ": " +
              item.kind +
              ", " +
              (item.graph === null || item.graph === undefined ? "all graphs" : "G" + item.graph) +
              ", " +
              (item.active ? "active" : "inactive"),
          ),
        )
      : [paragraph("No regions are defined.")],
  );
}

function defineRegion(context: Context): void {
  const project = context.project();
  if (!project) return;
  const choices = regionChoices(project);
  showDialog(
    "Define region",
    [
      field("Region", select("target", [["new", "New region"], ...choices], "new")),
      field("Type", select("kind", kinds, "inHorizontalRange")),
      field("Apply to", select("graph", graphChoices(project), String(context.activeGraph()))),
      checked("active", "Active", true),
      paragraph(
        "Polygon points use X, Y pairs separated by semicolons. Line fields apply to line regions. Range fields apply to range regions.",
      ),
      field("Polygon points", input("points", "0, 0; 1, 0; 1, 1")),
      field("First X", input("firstX", "0", "number")),
      field("First Y", input("firstY", "0", "number")),
      field("Second X", input("secondX", "1", "number")),
      field("Second Y", input("secondY", "1", "number")),
      field("Range start", input("start", "0", "number")),
      field("Range stop", input("stop", "1", "number")),
    ],
    (data) => {
      const next = region(data);
      const existing = formText(data, "target");
      return context.apply(
        existing === "new"
          ? { type: "regionOperation", operation: { method: "create", region: next } }
          : {
              type: "regionOperation",
              operation: { method: "replace", id: Number(existing), region: next },
            },
      );
    },
  );
}

function clearRegion(context: Context): void {
  const project = context.project();
  if (!project) return;
  const choices = regionChoices(project);
  if (!choices.length) {
    showDialog("Clear region", [paragraph("No regions are defined.")]);
    return;
  }
  showDialog("Clear region", [field("Region", select("region", choices, choices[0]![0]))], (data) =>
    context.apply({
      type: "regionOperation",
      operation: { method: "delete", id: integer(data, "region") },
    }),
  );
}

function reportRegion(context: Context): void {
  const project = context.project();
  if (!project) return;
  const choices = regionChoices(project);
  if (!choices.length) {
    showDialog("Report on region", [paragraph("No regions are defined.")]);
    return;
  }
  showDialog(
    "Report on region",
    [
      field("Region", select("region", choices, choices[0]![0])),
      field(
        "Graph",
        select("graph", graphChoices(project).slice(1), String(context.activeGraph())),
      ),
    ],
    async (data) => {
      const response = await engine({
        type: "regionReport",
        id: integer(data, "region"),
        graph: integer(data, "graph"),
      });
      if (response.type !== "regionReport")
        throw new Error("Grace did not return a region report.");
      const summary = response.sets.length
        ? response.sets.map(({ set, count }) => "S" + set + ": " + count + " points").join(", ")
        : "No points belong to this region.";
      // The dialog form closes after a successful submit; reopen after that
      // lifecycle completes so the report remains visible.
      window.setTimeout(() => showDialog("Region report", [paragraph(summary)]), 0);
    },
  );
}

export function regionDialogs(context: Context): Record<string, () => void> {
  return {
    "region-status": () => regionStatus(context),
    "region-define": () => defineRegion(context),
    "region-clear": () => clearRegion(context),
    "region-report": () => reportRegion(context),
  };
}

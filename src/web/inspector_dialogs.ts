import { field, input, paragraph, select, showDialog } from "./dialog";
import { engine } from "./engine";
import type { Command, Project } from "./protocol";

type Context = {
  project: () => Project | undefined;
  activeGraph: () => number;
  activeSet: () => number;
  apply: (command: Command) => Promise<void>;
  download: (blob: Blob, name: string) => void;
};

type PointState = { graph: number; set: number; row: number };

const symbols: readonly [string, string][] = [
  ["\\x", "Greek and Symbol font"],
  ["\\xe", "epsilon"],
  ["\\xa", "alpha"],
  ["\\xb", "beta"],
  ["\\xg", "gamma"],
  ["\\xp", "pi"],
  ["\\xD", "Delta"],
  ["\\xS", "Sigma"],
  ["\\xW", "Omega"],
  ["\\s", "Begin subscript"],
  ["\\S", "Begin superscript"],
  ["\\N", "Return to normal baseline"],
  ["\\f{Times-Bold}", "Times Bold font"],
  ["\\f{Helvetica}", "Helvetica font"],
  ["\\+", "Increase size"],
  ["\\-", "Decrease size"],
  ["\\q", "Begin oblique text"],
  ["\\Q", "End oblique text"],
  ["\\\\", "Literal backslash"],
];

function project(context: Context): Project {
  const current = context.project();
  if (!current) throw new Error("Open or create a Grace project first.");
  return current;
}

function integer(value: string, label: string, maximum: number): number {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0 || number > maximum)
    throw new Error(`Enter a ${label} from 0 through ${maximum}.`);
  return number;
}

function pointExplorer(context: Context): void {
  const current = context.project();
  if (!current) {
    showDialog("Point explorer", [paragraph("Open or create a Grace project first.")]);
    return;
  }
  const graph = context.activeGraph();
  const sets = current.graphs[String(graph)]?.sets ?? {};
  const choices = Object.entries(sets).map(([id, set]) => [id, `S${id} [${set.count}]`] as const);
  if (!choices.length) {
    showDialog("Point explorer", [paragraph("The active graph has no data sets.")]);
    return;
  }
  const firstChoice = choices[0];
  if (!firstChoice) return;
  const state: PointState = {
    graph,
    set: Number(choices.find(([id]) => Number(id) === context.activeSet())?.[0] ?? firstChoice[0]),
    row: 0,
  };

  const render = async (): Promise<void> => {
    const info = project(context).graphs[String(state.graph)]?.sets[String(state.set)];
    if (!info || info.count === 0) throw new Error("Select a non-empty data set.");
    state.row = Math.min(state.row, info.count - 1);
    const result = await engine({
      type: "dataTable",
      graph: state.graph,
      set: state.set,
      offset: state.row,
      limit: 1,
    });
    if (result.type !== "dataTable" || !result.rows[0])
      throw new Error("Grace did not return the selected point.");
    const values = result.rows[0];
    const graphControl = select(
      "graph",
      Object.keys(project(context).graphs).map((id) => [id, `G${id}`] as const),
      String(state.graph),
    );
    const setControl = select(
      "set",
      Object.entries(project(context).graphs[String(state.graph)]?.sets ?? {}).map(
        ([id, set]) => [id, `S${id} [${set.count}]`] as const,
      ),
      String(state.set),
    );
    const rowControl = input("row", String(state.row), "number");
    rowControl.min = "0";
    rowControl.max = String(info.count - 1);
    const previous = document.createElement("button");
    previous.type = "button";
    previous.textContent = "Previous";
    previous.disabled = state.row === 0;
    const next = document.createElement("button");
    next.type = "button";
    next.textContent = "Next";
    next.disabled = state.row + 1 >= info.count;
    const update = (): void => {
      try {
        state.graph = integer(graphControl.value, "graph", Number.MAX_SAFE_INTEGER);
        const selected = project(context).graphs[graphControl.value];
        if (!selected) throw new Error("Select an existing graph.");
        state.set = integer(setControl.value, "set", Number.MAX_SAFE_INTEGER);
        const set = selected.sets[setControl.value];
        if (!set) throw new Error("Select an existing set.");
        state.row = integer(rowControl.value, "row", set.count - 1);
        open();
      } catch (error) {
        showDialog("Point explorer", [
          paragraph(error instanceof Error ? error.message : "Invalid point."),
        ]);
      }
    };
    graphControl.addEventListener("change", () => {
      state.graph = Number(graphControl.value);
      state.set = Number(
        Object.keys(project(context).graphs[graphControl.value]?.sets ?? {})[0] ?? 0,
      );
      state.row = 0;
      open();
    });
    setControl.addEventListener("change", () => {
      state.set = Number(setControl.value);
      state.row = 0;
      open();
    });
    rowControl.addEventListener("change", update);
    previous.addEventListener("click", () => {
      state.row -= 1;
      open();
    });
    next.addEventListener("click", () => {
      state.row += 1;
      open();
    });
    const coordinates = values
      .map(
        (value, index) => `${index === 0 ? "X" : index === 1 ? "Y" : `Y${index - 1}`} = ${value}`,
      )
      .join("   ");
    showDialog("Point explorer", [
      field("Graph", graphControl),
      field("Set", setControl),
      field("Point", rowControl),
      paragraph(coordinates),
      previous,
      next,
    ]);
  };
  const open = (): void => {
    void render().catch((error: unknown) => {
      showDialog("Point explorer", [
        paragraph(error instanceof Error ? error.message : "Unable to inspect this point."),
      ]);
    });
  };
  open();
}

function fontTool(): void {
  const firstSymbol = symbols[0];
  if (!firstSymbol) return;
  const escape = select("escape", symbols, firstSymbol[0]);
  const preview = paragraph("");
  preview.className = "font-tool-preview";
  const copy = document.createElement("button");
  copy.type = "button";
  copy.textContent = "Copy escape";
  const refresh = (): void => {
    preview.textContent = `Selected escape: ${escape.value}`;
  };
  copy.addEventListener("click", () => {
    if (!navigator.clipboard) {
      copy.textContent = "Select code to copy";
      return;
    }
    void navigator.clipboard.writeText(escape.value).then(
      () => {
        copy.textContent = "Copied";
      },
      () => {
        copy.textContent = "Copy unavailable";
      },
    );
  });
  escape.addEventListener("change", refresh);
  refresh();
  showDialog("Font tool", [
    paragraph(
      "Choose a Grace text escape and copy it into a title, label, legend, or drawing text field.",
    ),
    field("Character or style", escape),
    preview,
    copy,
  ]);
}

export function inspectorDialogs(context: Context): Record<string, () => void> {
  return {
    "point-explorer": () => pointExplorer(context),
    "font-tool": fontTool,
  };
}

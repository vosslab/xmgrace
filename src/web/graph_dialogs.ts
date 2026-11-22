import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import type { Bounds, Command, Project } from "./protocol";

type Context = {
  project: () => Project | undefined;
  activeGraph: () => number;
  apply: (command: Command) => Promise<void>;
};

function number(data: FormData, name: string): number {
  const value = Number(data.get(name));
  if (!Number.isFinite(value)) throw new Error(`Enter a finite ${name}.`);
  return value;
}

function integer(data: FormData, name: string, minimum: number): number {
  const value = number(data, name);
  if (!Number.isInteger(value) || value < minimum)
    throw new Error(`Enter a ${name} of at least ${minimum}.`);
  return value;
}

function graphChoices(project: Project): [string, string][] {
  return Object.entries(project.graphs).map(([id, graph]) => [
    id,
    `G${id}${graph.visible ? "" : " (hidden)"}`,
  ]);
}

function checkbox(name: string, label: string, checked: boolean): HTMLLabelElement {
  const control = input(name, "on", "checkbox");
  control.checked = checked;
  const wrapper = document.createElement("label");
  wrapper.append(control, label);
  return wrapper;
}

function selectedProject(context: Context): Project {
  const project = context.project();
  if (!project) throw new Error("Open or create a Grace project first.");
  return project;
}

function manageGraphs(context: Context): void {
  const project = selectedProject(context);
  const choices = graphChoices(project);
  const active = String(context.activeGraph());
  const available = choices.find(([id]) => id === active)?.[0] ?? choices[0]?.[0] ?? "0";
  showDialog(
    "Graphs",
    [
      field(
        "Action",
        select(
          "action",
          [
            ["create", "New graph"],
            ["duplicate", "Duplicate graph"],
            ["show", "Show graph"],
            ["hide", "Hide graph"],
            ["delete", "Delete graph"],
          ],
          "create",
        ),
      ),
      field("Graph", select("graph", choices, available)),
      paragraph(
        "New graphs use the current Grace page. Delete removes the selected graph and its sets.",
      ),
    ],
    (data) => {
      const action = formText(data, "action");
      const graph = integer(data, "graph", 0);
      if (action === "create")
        return context.apply({ type: "graphOperation", operation: { method: "create" } });
      if (action === "duplicate")
        return context.apply({ type: "graphOperation", operation: { method: "duplicate", graph } });
      if (action === "delete")
        return context.apply({ type: "graphOperation", operation: { method: "delete", graph } });
      return context.apply({
        type: "graphOperation",
        operation: { method: "visibility", graph, visible: action === "show" },
      });
    },
  );
}

function arrangeGraphs(context: Context): void {
  selectedProject(context);
  showDialog(
    "Arrange graphs",
    [
      field("Rows", input("rows", "1", "number")),
      field("Columns", input("columns", "1", "number")),
      field("Gap", input("gap", "0.1", "number")),
    ],
    (data) =>
      context.apply({
        type: "graphOperation",
        operation: {
          method: "arrange",
          rows: integer(data, "rows", 1),
          columns: integer(data, "columns", 1),
          gap: number(data, "gap"),
        },
      }),
  );
}

function overlayGraphs(context: Context): void {
  const project = selectedProject(context);
  const choices = graphChoices(project);
  if (choices.length < 2) throw new Error("Create a second graph before overlaying graphs.");
  const active = String(context.activeGraph());
  const source = choices.find(([id]) => id !== active)?.[0] ?? choices[0]?.[0] ?? "0";
  showDialog(
    "Overlay graphs",
    [
      field("Graph", select("graph", choices, active)),
      field("Use world limits from", select("other", choices, source)),
      checkbox("sharedX", "Share X limits", true),
      checkbox("sharedY", "Share Y limits", true),
    ],
    (data) => {
      const graph = integer(data, "graph", 0);
      const other = integer(data, "other", 0);
      if (graph === other) throw new Error("Select two different graphs.");
      return context.apply({
        type: "graphOperation",
        operation: {
          method: "overlay",
          graph,
          other,
          sharedX: data.has("sharedX"),
          sharedY: data.has("sharedY"),
        },
      });
    },
  );
}

function viewport(context: Context): void {
  const project = selectedProject(context);
  const choices = graphChoices(project);
  const active = String(context.activeGraph());
  const graph = project.graphs[active] ?? project.graphs[choices[0]?.[0] ?? "0"];
  if (!graph) throw new Error("Select an existing graph.");
  showDialog(
    "Graph viewport",
    [
      field("Graph", select("graph", choices, active)),
      field("Left", input("left", String(graph.view[0]), "number")),
      field("Bottom", input("bottom", String(graph.view[1]), "number")),
      field("Right", input("right", String(graph.view[2]), "number")),
      field("Top", input("top", String(graph.view[3]), "number")),
    ],
    (data) => {
      const bounds: Bounds = [
        number(data, "left"),
        number(data, "bottom"),
        number(data, "right"),
        number(data, "top"),
      ];
      if (bounds[0] >= bounds[2] || bounds[1] >= bounds[3])
        throw new Error("Viewport limits must increase.");
      return context.apply({
        type: "graphOperation",
        operation: { method: "viewport", graph: integer(data, "graph", 0), bounds },
      });
    },
  );
}

function pageSetup(context: Context): void {
  const project = selectedProject(context);
  showDialog(
    "Page setup",
    [
      field("Width", input("width", String(project.page[0]), "number")),
      field("Height", input("height", String(project.page[1]), "number")),
      field("Background color", input("background", String(project.background), "number")),
      checkbox("fill", "Fill page background", project.fill_page),
    ],
    (data) => {
      const width = number(data, "width");
      const height = number(data, "height");
      const background = integer(data, "background", 0);
      if (width <= 0 || height <= 0) throw new Error("Enter a valid page size and color.");
      return context.apply({
        type: "graphOperation",
        operation: { method: "page", width, height, background, fill: data.has("fill") },
      });
    },
  );
}

export function graphDialogs(context: Context): Record<string, () => void> {
  return {
    "manage-graphs": () => manageGraphs(context),
    "arrange-graphs": () => arrangeGraphs(context),
    "overlay-graphs": () => overlayGraphs(context),
    viewport: () => viewport(context),
    "page-setup": () => pageSetup(context),
  };
}

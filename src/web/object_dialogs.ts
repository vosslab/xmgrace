import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import type { Command, DrawingObject, Project } from "./protocol";

type Context = {
  project: () => Project | undefined;
  activeGraph: () => number;
  apply: (command: Command) => Promise<void>;
};

type ObjectKind = DrawingObject["kind"];
type ObjectOperation = Extract<Command, { type: "objectOperation" }>["operation"];
type TextStyle = Extract<DrawingObject, { kind: "text" }>["style"];
type Stroke = Extract<DrawingObject, { kind: "line" }>["stroke"];
type Fill = Extract<DrawingObject, { kind: "box" | "ellipse" }>["fill"];
type Arrow = Extract<DrawingObject, { kind: "line" }>["arrow"];

function number(data: FormData, name: string): number {
  const value = Number(data.get(name));
  if (!Number.isFinite(value)) throw new Error(`Enter a finite ${name}.`);
  return value;
}

function integer(data: FormData, name: string, minimum = 0): number {
  const value = number(data, name);
  if (!Number.isInteger(value) || value < minimum)
    throw new Error(`Enter an integer ${name} of at least ${minimum}.`);
  return value;
}

function checkbox(name: string, label: string, checked: boolean): HTMLLabelElement {
  const control = input(name, "on", "checkbox");
  control.checked = checked;
  const wrapper = document.createElement("label");
  wrapper.className = "check-field";
  wrapper.append(control, label);
  return wrapper;
}

function objectChoices(project: Project): readonly (readonly [string, string])[] {
  return Object.entries(project.objects).map(([id, object]) => [
    id,
    `${id}: ${object.kind}${object.visible ? "" : " (hidden)"}`,
  ]);
}

function objectKind(value: string): ObjectKind {
  if (value === "text" || value === "line" || value === "box" || value === "ellipse") return value;
  throw new Error("Select a drawing-object type.");
}

function visible(object: DrawingObject | undefined): boolean {
  return object?.visible ?? true;
}

function coordinates(object: DrawingObject | undefined): number[] {
  return object?.coordinates ?? [0, 0, 1, 1];
}

function space(object: DrawingObject | undefined): "view" | "world" {
  return object?.space.space ?? "view";
}

function worldGraph(object: DrawingObject | undefined, activeGraph: number): number {
  return object?.space.space === "world" ? object.space.graph : activeGraph;
}

function kind(object: DrawingObject | undefined): ObjectKind {
  return object?.kind ?? "text";
}

function textStyle(object: DrawingObject | undefined): TextStyle {
  return object?.kind === "text"
    ? object.style
    : { color: 1, rotation: 0, font: 0, justification: 0, size: 1 };
}

function stroke(object: DrawingObject | undefined): Stroke {
  return object?.kind === "text"
    ? { color: 1, width: 1, style: 1 }
    : (object?.stroke ?? { color: 1, width: 1, style: 1 });
}

function fill(object: DrawingObject | undefined): Fill {
  return object?.kind === "box" || object?.kind === "ellipse"
    ? object.fill
    : { color: 1, pattern: 0 };
}

function arrow(object: DrawingObject | undefined): Arrow {
  return object?.kind === "line"
    ? object.arrow
    : { ends: 0, kind: 1, size: 1, layout: [1, 1] as [number, number] };
}

function initialObject(data: FormData): DrawingObject {
  const selectedKind = objectKind(formText(data, "kind"));
  const coordinateCount = selectedKind === "text" ? 2 : 4;
  const coordinates = Array.from({ length: coordinateCount }, (_, index) =>
    number(data, `coordinate${index}`),
  );
  const selectedSpace = formText(data, "space");
  const objectSpace =
    selectedSpace === "world"
      ? { space: "world" as const, graph: integer(data, "worldGraph") }
      : { space: "view" as const };
  const isVisible = data.has("visible");
  if (selectedKind === "text") {
    return {
      kind: "text",
      visible: isVisible,
      coordinates: [coordinates[0]!, coordinates[1]!],
      space: objectSpace,
      text: formText(data, "text"),
      style: {
        color: integer(data, "textColor"),
        rotation: number(data, "rotation"),
        font: integer(data, "font"),
        justification: integer(data, "justification"),
        size: number(data, "textSize"),
      },
    };
  }
  const common = {
    visible: isVisible,
    coordinates: [coordinates[0]!, coordinates[1]!, coordinates[2]!, coordinates[3]!] as [
      number,
      number,
      number,
      number,
    ],
    space: objectSpace,
    stroke: {
      color: integer(data, "lineColor"),
      width: number(data, "lineWidth"),
      style: integer(data, "lineStyle"),
    },
  };
  if (selectedKind === "line") {
    return {
      kind: "line",
      ...common,
      arrow: {
        ends: integer(data, "arrowEnds"),
        kind: integer(data, "arrowKind"),
        size: number(data, "arrowSize"),
        layout: [number(data, "arrowWidth"), number(data, "arrowNotch")],
      },
    };
  }
  return {
    kind: selectedKind,
    ...common,
    fill: { color: integer(data, "fillColor"), pattern: integer(data, "fillPattern") },
  };
}

function objectFields(object: DrawingObject | undefined, activeGraph: number): HTMLElement[] {
  const selectedKind = kind(object);
  const values = coordinates(object);
  const selectedSpace = space(object);
  const isText = selectedKind === "text";
  const selectedStroke = stroke(object);
  const selectedFill = fill(object);
  const selectedArrow = arrow(object);
  const selectedText = textStyle(object);
  const fields: HTMLElement[] = [
    field(
      "Type",
      select(
        "kind",
        [
          ["text", "Text"],
          ["line", "Line or arrow"],
          ["box", "Box"],
          ["ellipse", "Ellipse"],
        ],
        selectedKind,
      ),
    ),
    checkbox("visible", "Visible", visible(object)),
    field(
      "Coordinate system",
      select(
        "space",
        [
          ["view", "View"],
          ["world", "World"],
        ],
        selectedSpace,
      ),
    ),
    field("World graph", input("worldGraph", String(worldGraph(object, activeGraph)), "number")),
    field(isText ? "X" : "X1", input("coordinate0", String(values[0] ?? 0), "number")),
    field(isText ? "Y" : "Y1", input("coordinate1", String(values[1] ?? 0), "number")),
  ];
  if (isText) {
    fields.push(
      field("Text", input("text", object?.kind === "text" ? object.text : "")),
      field("Color", input("textColor", String(selectedText.color), "number")),
      field("Rotation", input("rotation", String(selectedText.rotation), "number")),
      field("Font", input("font", String(selectedText.font), "number")),
      field("Justification", input("justification", String(selectedText.justification), "number")),
      field("Character size", input("textSize", String(selectedText.size), "number")),
    );
    return fields;
  }
  fields.push(
    field("X2", input("coordinate2", String(values[2] ?? 1), "number")),
    field("Y2", input("coordinate3", String(values[3] ?? 1), "number")),
    field("Line color", input("lineColor", String(selectedStroke.color), "number")),
    field("Line width", input("lineWidth", String(selectedStroke.width), "number")),
    field("Line style", input("lineStyle", String(selectedStroke.style), "number")),
  );
  if (selectedKind === "line") {
    fields.push(
      field(
        "Arrow ends",
        select(
          "arrowEnds",
          [
            ["0", "None"],
            ["1", "First"],
            ["2", "Second"],
            ["3", "Both"],
          ],
          String(selectedArrow.ends),
        ),
      ),
      field(
        "Arrow type",
        select(
          "arrowKind",
          [
            ["0", "Open"],
            ["1", "Filled"],
            ["2", "Hollow"],
          ],
          String(selectedArrow.kind),
        ),
      ),
      field("Arrow size", input("arrowSize", String(selectedArrow.size), "number")),
      field("Arrow width", input("arrowWidth", String(selectedArrow.layout[0]), "number")),
      field("Arrow notch", input("arrowNotch", String(selectedArrow.layout[1]), "number")),
    );
  } else {
    fields.push(
      field("Fill color", input("fillColor", String(selectedFill.color), "number")),
      field("Fill pattern", input("fillPattern", String(selectedFill.pattern), "number")),
    );
  }
  return fields;
}

function newObject(selectedKind: ObjectKind, activeGraph: number): DrawingObject {
  const space = { space: "world" as const, graph: activeGraph };
  if (selectedKind === "text")
    return {
      kind: "text",
      visible: true,
      coordinates: [0, 0],
      space,
      text: "",
      style: { color: 1, rotation: 0, font: 0, justification: 0, size: 1 },
    };
  const common = {
    visible: true,
    coordinates: [0, 0, 1, 1] as [number, number, number, number],
    space,
    stroke: { color: 1, width: 1, style: 1 },
  };
  if (selectedKind === "line")
    return {
      kind: "line",
      ...common,
      arrow: { ends: 0, kind: 1, size: 1, layout: [1, 1] as [number, number] },
    };
  return { kind: selectedKind, ...common, fill: { color: 1, pattern: 0 } };
}

function edit(context: Context, id: number | undefined, object: DrawingObject): void {
  showDialog(
    `${id === undefined ? "Create" : "Edit"} drawing object`,
    objectFields(object, context.activeGraph()),
    (data) => {
      const operation: ObjectOperation =
        id === undefined
          ? { method: "create", object: initialObject(data) }
          : { method: "replace", id, object: initialObject(data) };
      return context.apply({ type: "objectOperation", operation });
    },
  );
}

function manage(context: Context): void {
  const project = context.project();
  if (!project) {
    showDialog("Drawing objects", [paragraph("Open or create a Grace project first.")]);
    return;
  }
  const choices = objectChoices(project);
  const fields: HTMLElement[] = [
    field(
      "Action",
      select(
        "action",
        [
          ["create", "Create"],
          ["edit", "Edit"],
          ["show", "Show"],
          ["hide", "Hide"],
          ["delete", "Delete"],
        ],
        choices.length ? "edit" : "create",
      ),
    ),
    field(
      "Object",
      select("object", choices.length ? choices : [["0", "No object"]], choices[0]?.[0] ?? "0"),
    ),
    field(
      "New object type",
      select(
        "createKind",
        [
          ["text", "Text"],
          ["line", "Line or arrow"],
          ["box", "Box"],
          ["ellipse", "Ellipse"],
        ],
        "text",
      ),
    ),
  ];
  showDialog("Drawing objects", fields, (data) => {
    const action = formText(data, "action");
    if (action === "create") {
      const object = newObject(objectKind(formText(data, "createKind")), context.activeGraph());
      window.setTimeout(() => edit(context, undefined, object));
      return;
    }
    const id = integer(data, "object");
    if (action === "edit") {
      const object = project.objects[String(id)];
      if (!object) throw new Error("Select an existing drawing object.");
      window.setTimeout(() => edit(context, id, object));
      return;
    }
    const operation: ObjectOperation =
      action === "delete"
        ? { method: "delete", id }
        : { method: "visibility", id, visible: action === "show" };
    return context.apply({ type: "objectOperation", operation });
  });
}

export function objectDialogs(context: Context): Record<string, () => void> {
  return { "drawing-objects": () => manage(context) };
}

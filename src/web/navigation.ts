import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import type { Bounds, Command, Graph, Project } from "./protocol";
import type { Preferences } from "./preferences";

type ZoomAxis = "xy" | "x" | "y";

type Context = {
  graph: () => Graph | undefined;
  project: () => Project | undefined;
  activeGraph: () => number;
  editable: () => boolean;
  apply: (command: Command) => Promise<void>;
  report: (error: unknown) => void;
  selectGraph: (id: number) => void;
  preferences: () => Preferences;
};

type Point = { x: number; y: number; id: number };
type State = { cursor: number; worlds: Bounds[] };
type LocatorMode = "xy" | "delta" | "distance" | "polar" | "view" | "screen";
type LocatorState = {
  mode: LocatorMode;
  fixed?: [number, number];
};

const locatorModes: readonly [LocatorMode, string][] = [
  ["xy", "[X, Y]"],
  ["delta", "[DX, DY]"],
  ["distance", "[DISTANCE]"],
  ["polar", "[Phi, Rho]"],
  ["view", "[VX, VY]"],
  ["screen", "[SX, SY]"],
];

const canvas = document.getElementById("canvas") as HTMLDivElement;
const plot = document.getElementById("plot") as HTMLImageElement;
const band = document.getElementById("rubber-band") as HTMLDivElement;
const locator = document.getElementById("locator") as HTMLOutputElement;
const stackOutput = document.getElementById("world-stack") as HTMLOutputElement | null;

function copy(bounds: Bounds): Bounds {
  return [bounds[0], bounds[1], bounds[2], bounds[3]];
}

function valid(bounds: Bounds): boolean {
  return bounds.every(Number.isFinite) && bounds[0] < bounds[2] && bounds[1] < bounds[3];
}

function scaleValue(graph: Graph, axis: 0 | 1, fraction: number): number {
  const start = axis === 0 ? graph.world[0] : graph.world[1];
  const stop = axis === 0 ? graph.world[2] : graph.world[3];
  const name = axis === 0 ? "x" : "y";
  const t = graph.settings[`${name}axes invert`] === "on" ? 1 - fraction : fraction;
  switch (graph.settings[`${name}axes scale`]?.toLowerCase()) {
    case "logarithmic":
      return Math.exp((1 - t) * Math.log(start) + t * Math.log(stop));
    case "reciprocal":
      return 1 / ((1 - t) / start + t / stop);
    case "logit": {
      const logit = (value: number): number => Math.log(value / (1 - value));
      const coordinate = (1 - t) * logit(start) + t * logit(stop);
      return 1 / (1 + Math.exp(-coordinate));
    }
  }
  return (1 - t) * start + t * stop;
}

function pagePosition(
  event: { clientX: number; clientY: number },
  project: Project,
): [number, number] | undefined {
  const rect = plot.getBoundingClientRect();
  if (!rect.width || !rect.height) return undefined;
  const [width, height] = project.page;
  const scale = Math.min(width, height);
  const x = (((event.clientX - rect.left) / rect.width) * width) / scale;
  const y = ((1 - (event.clientY - rect.top) / rect.height) * height) / scale;
  return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : undefined;
}

function graphAt(project: Project, point: [number, number]): number | undefined {
  const candidates = Object.entries(project.graphs)
    .filter(([, graph]) => {
      const [left, bottom, right, top] = graph.view;
      return (
        graph.visible &&
        point[0] >= left &&
        point[0] <= right &&
        point[1] >= bottom &&
        point[1] <= top
      );
    })
    .map(([id]) => Number(id));
  return candidates[candidates.length - 1];
}

export function navigation(context: Context): {
  actions: Record<string, () => void | Promise<void>>;
  reset: () => void;
  cancel: () => void;
  sync: () => void;
} {
  const states = new Map<number, State>();
  const locators = new Map<number, LocatorState>();
  let zoomAxis: ZoomAxis | undefined;
  let pickSet = false;
  let pickLocator = false;
  let drag: Point | undefined;

  function currentState(): State {
    const graph = context.graph();
    const id = context.activeGraph();
    const state = states.get(id);
    if (state) return state;
    const created = { cursor: 0, worlds: graph ? [copy(graph.world)] : [] };
    states.set(id, created);
    return created;
  }

  function renderStack(): void {
    if (!stackOutput) return;
    const state = currentState();
    stackOutput.textContent = `SD:${state.worlds.length} CW:${state.cursor + 1}`;
  }

  function currentLocator(): LocatorState {
    const id = context.activeGraph();
    const existing = locators.get(id);
    if (existing) return existing;
    const created: LocatorState = { mode: "xy" };
    locators.set(id, created);
    return created;
  }

  function text(value: number): string {
    return value.toPrecision(context.preferences().locatorPrecision);
  }

  function renderLocator(
    graph: Graph,
    world: [number, number],
    view: [number, number],
    screen: [number, number],
  ): void {
    const state = currentLocator();
    const origin = state.fixed ?? [0, 0];
    const [x, y] = world;
    let values: number[];
    switch (state.mode) {
      case "delta":
        values = [x - origin[0], y - origin[1]];
        break;
      case "distance":
        values = [Math.hypot(x - origin[0], y - origin[1])];
        break;
      case "polar":
        values = [
          Math.atan2(y - origin[1], x - origin[0]),
          Math.hypot(x - origin[0], y - origin[1]),
        ];
        break;
      case "view":
        values = view;
        break;
      case "screen":
        values = screen;
        break;
      case "xy":
        values = graph.kind === "Polar" ? [x * Math.cos(y), x * Math.sin(y)] : [x, y];
        break;
    }
    const label = locatorModes.find(([mode]) => mode === state.mode)?.[1] ?? "[X, Y]";
    locator.textContent = `G${context.activeGraph()}: ${label} = [${values.map(text).join(", ")}]`;
  }

  function locatorProperties(): void {
    const state = currentLocator();
    const [fixedX, fixedY] = state.fixed ?? [0, 0];
    showDialog(
      "Locator props",
      [
        field("Locator display type", select("mode", locatorModes, state.mode)),
        paragraph("Fixed point"),
        ((): HTMLLabelElement => {
          const control = document.createElement("input");
          control.name = "fixed";
          control.type = "checkbox";
          control.checked = state.fixed !== undefined;
          return field("Enable", control);
        })(),
        field("X", input("x", String(fixedX), "number")),
        field("Y", input("y", String(fixedY), "number")),
      ],
      (data) => {
        const mode = formText(data, "mode") as LocatorMode;
        if (!locatorModes.some(([candidate]) => candidate === mode))
          throw new Error("Select a locator display type.");
        let fixed: [number, number] | undefined;
        if (data.has("fixed")) {
          fixed = [Number(data.get("x")), Number(data.get("y"))];
          if (!fixed.every(Number.isFinite))
            throw new Error("Enter finite fixed-point coordinates.");
        }
        state.mode = mode;
        state.fixed = fixed;
      },
    );
  }

  function setZoom(axis: ZoomAxis | undefined): void {
    pickSet = false;
    pickLocator = false;
    zoomAxis = axis;
    drag = undefined;
    band.hidden = true;
    canvas.classList.toggle("zoom", axis !== undefined);
    document.querySelector('button[data-action="auto-on"]')?.setAttribute("aria-pressed", "false");
    for (const [name, value] of [
      ["zoom", "xy"],
      ["zoom-x", "x"],
      ["zoom-y", "y"],
    ] as const)
      document
        .querySelector<HTMLButtonElement>(`button[data-action="${name}"]`)
        ?.setAttribute("aria-pressed", String(axis === value));
  }

  function graphFraction(
    event: { clientX: number; clientY: number },
    graph: Graph,
  ): [number, number] | undefined {
    const project = context.project();
    if (!project) return undefined;
    const page = pagePosition(event, project);
    if (!page) return undefined;
    const [left, bottom, right, top] = graph.view;
    const x = (page[0] - left) / (right - left);
    const y = (page[1] - bottom) / (top - bottom);
    return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : undefined;
  }

  function applyWorld(bounds: Bounds): void {
    if (!context.editable() || !valid(bounds)) return;
    const graph = context.graph();
    if (!graph) return;
    void context
      .apply({ type: "world", graph: context.activeGraph(), bounds })
      .catch(context.report);
  }

  function zoomTo(first: [number, number], last: [number, number], axis: ZoomAxis): void {
    const graph = context.graph();
    if (!graph) return;
    const x0 = scaleValue(graph, 0, first[0]);
    const x1 = scaleValue(graph, 0, last[0]);
    const y0 = scaleValue(graph, 1, first[1]);
    const y1 = scaleValue(graph, 1, last[1]);
    const bounds = copy(graph.world);
    if (axis !== "y") [bounds[0], bounds[2]] = [Math.min(x0, x1), Math.max(x0, x1)];
    if (axis !== "x") [bounds[1], bounds[3]] = [Math.min(y0, y1), Math.max(y0, y1)];
    applyWorld(bounds);
  }

  function zoomBy(direction: number): void {
    const inset = (context.preferences().zoomPercent * direction) / 100;
    zoomTo([inset, inset], [1 - inset, 1 - inset], "xy");
  }

  function scroll(horizontal: number, vertical: number): void {
    const percent = context.preferences().scrollPercent / 100;
    const dx = horizontal * percent;
    const dy = vertical * percent;
    zoomTo([dx, dy], [1 + dx, 1 + dy], "xy");
  }

  canvas.addEventListener("pointermove", (event) => {
    const graph = context.graph();
    const fraction = graph && graphFraction(event, graph);
    if (graph && fraction) {
      const rect = canvas.getBoundingClientRect();
      renderLocator(
        graph,
        [scaleValue(graph, 0, fraction[0]), scaleValue(graph, 1, fraction[1])],
        fraction,
        [event.clientX - rect.left, event.clientY - rect.top],
      );
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
    if (event.button !== 0) return;
    canvas.focus({ preventScroll: true });
    const project = context.project();
    const page = project && pagePosition(event, project);
    const id = project && page && graphAt(project, page);
    if (id !== undefined && id !== context.activeGraph()) context.selectGraph(id);
    if (pickLocator && id !== undefined) {
      const selected = context.graph();
      const fraction = selected && graphFraction(event, selected);
      setZoom(undefined);
      if (selected && fraction)
        currentLocator().fixed = [
          scaleValue(selected, 0, fraction[0]),
          scaleValue(selected, 1, fraction[1]),
        ];
      return;
    }
    if (pickSet && id !== undefined) {
      const selected = context.graph();
      const fraction = selected && graphFraction(event, selected);
      setZoom(undefined);
      if (selected && fraction && context.editable())
        void context
          .apply({
            type: "autoOn",
            graph: id,
            x: scaleValue(selected, 0, fraction[0]),
            y: scaleValue(selected, 1, fraction[1]),
          })
          .catch(context.report);
      return;
    }
    if (!zoomAxis || id === undefined) return;
    drag = { x: event.clientX, y: event.clientY, id: event.pointerId };
    canvas.setPointerCapture(event.pointerId);
  });

  canvas.addEventListener("pointerup", (event) => {
    if (!drag || drag.id !== event.pointerId) return;
    const graph = context.graph();
    const start = graph && graphFraction({ clientX: drag.x, clientY: drag.y }, graph);
    const end = graph && graphFraction(event, graph);
    const distance = Math.hypot(event.clientX - drag.x, event.clientY - drag.y);
    const axis = zoomAxis;
    setZoom(undefined);
    if (start && end && axis && distance >= 6) zoomTo(start, end, axis);
  });
  canvas.addEventListener("pointercancel", () => setZoom(undefined));

  function autoOn(): void {
    const arm = !pickSet && context.editable();
    setZoom(undefined);
    pickSet = arm;
    canvas.classList.toggle("zoom", arm);
    document
      .querySelector('button[data-action="auto-on"]')
      ?.setAttribute("aria-pressed", String(arm));
  }

  function autoTicks(): void {
    if (!context.editable()) return;
    void context.apply({ type: "autoTicks", graph: context.activeGraph() }).catch(context.report);
  }

  function push(): void {
    const graph = context.graph();
    if (!graph) return;
    const state = currentState();
    if (state.worlds.length >= 128) {
      context.report(new Error("World stack full."));
      return;
    }
    state.worlds[state.cursor] = copy(graph.world);
    state.worlds.splice(state.cursor + 1, 0, copy(graph.world));
    renderStack();
  }

  function pop(): void {
    const graph = context.graph();
    const state = currentState();
    if (!graph || state.worlds.length <= 1) return;
    state.worlds.splice(state.cursor, 1);
    state.cursor = Math.min(state.cursor, state.worlds.length - 1);
    const next = state.worlds[state.cursor];
    if (next)
      void context
        .apply({ type: "world", graph: context.activeGraph(), bounds: copy(next) })
        .catch(context.report);
    renderStack();
  }

  function cycle(): void {
    const graph = context.graph();
    const state = currentState();
    if (!graph || state.worlds.length < 2) return;
    state.worlds[state.cursor] = copy(graph.world);
    state.cursor = (state.cursor + 1) % state.worlds.length;
    const next = state.worlds[state.cursor];
    if (next)
      void context
        .apply({ type: "world", graph: context.activeGraph(), bounds: copy(next) })
        .catch(context.report);
    renderStack();
  }

  return {
    actions: {
      zoom: () => setZoom(zoomAxis === "xy" ? undefined : "xy"),
      "zoom-x": () => setZoom(zoomAxis === "x" ? undefined : "x"),
      "zoom-y": () => setZoom(zoomAxis === "y" ? undefined : "y"),
      "zoom-in": () => zoomBy(1),
      "zoom-out": () => zoomBy(-1),
      left: () => scroll(-1, 0),
      right: () => scroll(1, 0),
      up: () => scroll(0, 1),
      down: () => scroll(0, -1),
      "auto-ticks": autoTicks,
      "auto-on": autoOn,
      "world-push-zoom": () => {
        push();
        setZoom("xy");
      },
      "world-push": push,
      "world-pop": pop,
      "world-cycle": cycle,
      "locator-set-fixed": () => {
        if (!context.graph()) return;
        setZoom(undefined);
        pickLocator = true;
        canvas.classList.add("zoom");
        canvas.focus({ preventScroll: true });
      },
      "locator-clear-fixed": () => {
        if (pickLocator) setZoom(undefined);
        currentLocator().fixed = undefined;
      },
      "locator-properties": locatorProperties,
    },
    reset: () => {
      states.clear();
      locators.clear();
      setZoom(undefined);
      renderStack();
    },
    cancel: () => setZoom(undefined),
    sync: renderStack,
  };
}

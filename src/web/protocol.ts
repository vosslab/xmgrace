export type Bounds = [number, number, number, number];
export type Settings = Record<string, string>;
export type Diagnostic = { message: string; line: number | null };
export type Graph = {
  world: Bounds;
  view: Bounds;
  settings: Settings;
  visible: boolean;
  sets: Record<string, { settings: Settings; count: number; kind: string }>;
};
export type Project = {
  page: [number, number];
  graphs: Record<string, Graph>;
  diagnostics: Diagnostic[];
};
export type Command =
  | { type: "autoscale"; graph: number; axes: "x" | "y" | "xy" }
  | { type: "world"; graph: number; bounds: Bounds }
  | {
      type: "axis";
      graph: number;
      axis: "x" | "y";
      min: number;
      max: number;
      major: number;
      label: string;
      log: boolean;
    }
  | { type: "graph"; graph: number; title: string; subtitle: string }
  | {
      type: "set";
      graph: number;
      set: number;
      label: string;
      color: number;
      symbol: number;
      line: number;
      hidden: boolean;
    };
export type Request =
  | { type: "new" | "state" | "save" }
  | { type: "open"; bytes: ArrayBuffer }
  | { type: "import"; graph: number; bytes: ArrayBuffer }
  | { type: "apply"; command: Command };
export type Result =
  | { type: "state"; project: Project; svg: string }
  | { type: "save"; bytes: Uint8Array<ArrayBuffer> }
  | { type: "error"; diagnostic: Diagnostic };
export type Message<T> = { id: number; body: T };

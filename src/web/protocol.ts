export type Bounds = [number, number, number, number];
export type Settings = Record<string, string>;
export type Diagnostic = { message: string; line: number | null };
export type SetKind =
  | "xy"
  | "bar"
  | "bardy"
  | "bardydy"
  | "xydx"
  | "xydy"
  | "xydxdx"
  | "xydydy"
  | "xydxdy"
  | "xydxdxdydy"
  | "xyhilo"
  | "xyr"
  | "xysize"
  | "xycolor"
  | "xycolpat"
  | "xyvmap"
  | "xyboxplot"
  | "xyz";
export type AxisScale = "normal" | "logarithmic" | "reciprocal" | "logit";
export type Graph = {
  world: Bounds;
  view: Bounds;
  settings: Settings;
  visible: boolean;
  kind: string;
  sets: Record<string, { settings: Settings; count: number; kind: string }>;
};
export type Project = {
  page: [number, number];
  graphs: Record<string, Graph>;
  diagnostics: Diagnostic[];
  console: string[];
  background: number;
  fill_page: boolean;
  objects: Record<string, DrawingObject>;
  regions: Record<string, Region>;
};
type Point = [number, number];
export type Region = { active?: boolean; graph?: number | null } & (
  | { kind: "insidePolygon" | "outsidePolygon"; points: Point[] }
  | { kind: "aboveLine" | "belowLine" | "leftOfLine" | "rightOfLine"; first: Point; second: Point }
  | {
      kind: "inHorizontalRange" | "outHorizontalRange" | "inVerticalRange" | "outVerticalRange";
      start: number;
      stop: number;
    }
);
type CoordinateSpace = { space: "view" } | { space: "world"; graph: number };
type ObjectStroke = { color: number; width: number; style: number };
export type DrawingObject =
  | {
      kind: "text";
      visible: boolean;
      coordinates: [number, number];
      space: CoordinateSpace;
      text: string;
      style: { color: number; rotation: number; font: number; justification: number; size: number };
    }
  | {
      kind: "line";
      visible: boolean;
      coordinates: Bounds;
      space: CoordinateSpace;
      stroke: ObjectStroke;
      arrow: { ends: number; kind: number; size: number; layout: [number, number] };
    }
  | {
      kind: "box" | "ellipse";
      visible: boolean;
      coordinates: Bounds;
      space: CoordinateSpace;
      stroke: ObjectStroke;
      fill: { color: number; pattern: number };
    };
export type FourierOptions = {
  algorithm: "fft" | "dft";
  input: "real" | "complex";
  direction: "forward" | "inverse";
  output: "magnitude" | "phase" | "coefficients" | "windowed";
  abscissa: "index" | "frequency" | "period";
  window: "none" | "triangular" | "hann" | "welch" | "hamming" | "blackman" | "parzen";
};
export type SeriesOperation =
  | {
      method: "histogram";
      start: number;
      stop: number;
      bins: number;
      cumulative: boolean;
      normalize: boolean;
    }
  | {
      method: "running";
      length: number;
      statistic: "mean" | "median" | "minimum" | "maximum" | "stddev";
    }
  | {
      method: "interpolate";
      start: number;
      stop: number;
      points: number;
      interpolation: "linear" | "spline" | "akima";
      strict: boolean;
    }
  | { method: "difference"; scheme: "forward" | "backward" | "centered" }
  | { method: "seasonalDifference"; period: number }
  | { method: "integrate" }
  | {
      method: "regression";
      degree: number;
      model?: "polynomial" | "power" | "exponential" | "logarithmic" | "reciprocal";
      output?: "fitted" | "residuals";
    };
export type SetOperation =
  | {
      method: "sort";
      set: number;
      axis: "x" | "y" | "y1" | "y2" | "y3" | "y4";
      descending: boolean;
    }
  | { method: "reverse" | "delete" | "duplicate" | "clear"; set: number }
  | { method: "join"; set: number; other: number }
  | { method: "split"; set: number; length: number }
  | { method: "drop"; set: number; first: number; last: number }
  | { method: "copy" | "move" | "swap"; set: number; targetGraph: number; targetSet?: number };
export type GraphOperation =
  | { method: "create" }
  | { method: "delete" | "duplicate"; graph: number }
  | { method: "visibility"; graph: number; visible: boolean }
  | { method: "viewport"; graph: number; bounds: Bounds }
  | { method: "arrange"; rows: number; columns: number; gap: number }
  | { method: "overlay"; graph: number; other: number; sharedX: boolean; sharedY: boolean }
  | { method: "page"; width: number; height: number; background: number; fill: boolean };
export type AdvancedOperation =
  | { method: "correlation"; maximumLag: number; covariance: boolean; unbiased: boolean }
  | { method: "digitalFilter" | "convolution" }
  | {
      method: "geometric";
      angle: number;
      origin: [number, number];
      scale: [number, number];
      translation: [number, number];
      order?:
        | "rotateTranslateScale"
        | "rotateScaleTranslate"
        | "translateScaleRotate"
        | "translateRotateScale"
        | "scaleTranslateRotate"
        | "scaleRotateTranslate";
    }
  | { method: "sampling"; start: number; step: number; condition?: string }
  | {
      method: "pruning";
      tolerance: number;
      kind?: "interpolation" | "circle" | "ellipse" | "rectangle";
      deltaX?: number;
      deltaY?: number;
    };
export type Feature =
  | "yMinimum"
  | "yMaximum"
  | "yAverage"
  | "yStandardDeviation"
  | "yMedian"
  | "xMinimum"
  | "xMaximum"
  | "xAverage"
  | "xStandardDeviation"
  | "xMedian"
  | "frequency"
  | "period"
  | "zeroCrossing"
  | "riseTime"
  | "fallTime"
  | "slope"
  | "yIntercept"
  | "setLength"
  | "halfMaximumWidth"
  | "barycenterX"
  | "barycenterY"
  | "xAtYMaximum"
  | "yAtXMaximum"
  | "integral";
export type Command =
  | {
      type: "generate";
      graph: number;
      start: string;
      stop: string;
      points: number;
      x: string;
      y: string;
    }
  | {
      type: "evaluate";
      graph: number;
      set: number;
      x: string;
      y: string;
      region?: number;
      negate?: boolean;
      replace?: boolean;
    }
  | { type: "fourier"; graph: number; set: number; options: FourierOptions }
  | { type: "series"; graph: number; set: number; operation: SeriesOperation }
  | { type: "advanced"; graph: number; set: number; other?: number; operation: AdvancedOperation }
  | {
      type: "fit";
      graph: number;
      set: number;
      options: {
        formula: string;
        parameters: number[];
        iterations: number;
        tolerance: number;
        lower?: number[];
        upper?: number[];
        output?: "fitted" | "residuals";
      };
    }
  | { type: "featureExtraction"; graph: number; feature: Feature }
  | { type: "setOperation"; graph: number; operation: SetOperation }
  | {
      type: "replaceData";
      graph: number;
      set: number | null;
      kind: SetKind;
      rows: number[][];
      expectedRevision?: number;
    }
  | { type: "graphOperation"; operation: GraphOperation }
  | {
      type: "regionOperation";
      operation:
        | { method: "create"; region: Region }
        | { method: "replace"; id: number; region: Region }
        | { method: "delete"; id: number }
        | { method: "visibility"; id: number; active: boolean };
    }
  | {
      type: "appearance";
      operation:
        | {
            method: "graph";
            graph: number;
            kind?: string;
            legend?: boolean;
            frame?: boolean;
            frameWidth?: number;
            titleSize?: number;
            subtitleSize?: number;
            stacked?: boolean;
            barGap?: number;
          }
        | {
            method: "set";
            graph: number;
            set: number;
            lineWidth?: number;
            lineStyle?: number;
            symbolSize?: number;
            symbolSkip?: number;
            errorBars?: boolean;
            errorSize?: number;
            fillType?: number;
            fillColor?: number;
            baselineType?: number;
            baseline?: boolean;
            dropLines?: boolean;
          }
        | {
            method: "axis";
            graph: number;
            axis: "x" | "y" | "alternateX" | "alternateY";
            visible?: boolean;
            invert?: boolean;
            grid?: boolean;
            minorGrid?: boolean;
            minorTicks?: number;
            precision?: number;
            format?: string;
            placement?: string;
            direction?: string;
            stagger?: number;
            label?: string;
          };
    }
  | {
      type: "objectOperation";
      operation:
        | { method: "create"; object: DrawingObject }
        | { method: "replace"; id: number; object: DrawingObject }
        | { method: "delete"; id: number }
        | { method: "visibility"; id: number; visible: boolean };
    }
  | { type: "autoscale"; graph: number; axes: "x" | "y" | "xy" }
  | { type: "world"; graph: number; bounds: Bounds }
  | { type: "autoTicks"; graph: number }
  | { type: "autoOn"; graph: number; x: number; y: number }
  | {
      type: "axis";
      graph: number;
      axis: "x" | "y";
      min: number;
      max: number;
      major: number;
      label: string;
      scale: AxisScale;
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
  | { type: "importMapped"; graph: number; bytes: ArrayBuffer; options: ImportOptions }
  | { type: "exportAscii"; graph: number; set: number }
  | { type: "dataTable"; graph: number; set: number; offset: number; limit: number }
  | { type: "reloadData"; graph: number; set: number; bytes: ArrayBuffer }
  | { type: "regionReport"; graph: number; id: number }
  | { type: "loadParameters"; graph: number; bytes: ArrayBuffer }
  | { type: "saveParameters"; graph: number }
  | { type: "netcdfVariables"; bytes: ArrayBuffer }
  | { type: "importNetcdf"; graph: number; bytes: ArrayBuffer; x: string; y: string }
  | { type: "script"; graph: number; input: string }
  | { type: "apply"; command: Command };
export type ImportOptions = {
  delimiter: "comma" | "tab" | "whitespace" | "auto";
  header: boolean;
  kind: SetKind;
  columns: (number | null)[];
  extraY?: number[];
  dateX?: boolean;
};
export type Result =
  | { type: "state"; project: Project; svg: string }
  | { type: "save"; bytes: Uint8Array<ArrayBuffer> }
  | { type: "ascii"; bytes: Uint8Array<ArrayBuffer> }
  | { type: "parameters"; bytes: Uint8Array<ArrayBuffer> }
  | { type: "dataTable"; rows: number[][]; revision: number }
  | { type: "regionReport"; sets: { set: number; count: number }[] }
  | { type: "netcdfVariables"; variables: [string, number][] }
  | { type: "error"; diagnostic: Diagnostic };
export type Message<T> = { id: number; body: T };

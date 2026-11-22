import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import type { AdvancedOperation, Command, Feature, Graph } from "./protocol";

type Context = {
  graph: () => Graph | undefined;
  activeGraph: () => number;
  apply: (command: Command) => Promise<void>;
};

type SetChoice = readonly [string, string];
type GeometricOrder = Extract<AdvancedOperation, { method: "geometric" }>["order"];

function integer(data: FormData, name: string, minimum = 0): number {
  const value = Number(data.get(name));
  if (!Number.isInteger(value) || value < minimum)
    throw new Error(`Enter an integer ${name} of at least ${minimum}.`);
  return value;
}

function number(data: FormData, name: string): number {
  const value = Number(data.get(name));
  if (!Number.isFinite(value)) throw new Error(`Enter a finite ${name}.`);
  return value;
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

function setChoices(context: Context): SetChoice[] {
  return Object.entries(context.graph()?.sets ?? {})
    .filter(([, set]) => set.count > 0)
    .map(([id, set]) => [id, `G${context.activeGraph()}.S${id}  (${set.count} points)`]);
}

function selectedSet(context: Context, title: string, name = "set"): HTMLSelectElement | undefined {
  const choices = setChoices(context);
  if (!choices.length) {
    showDialog(title, [paragraph("Create or import a data set first.")]);
    return undefined;
  }
  return select(name, choices, choices[0]![0]);
}

function advanced(
  context: Context,
  data: FormData,
  operation: AdvancedOperation,
  other?: number,
): Promise<void> {
  return context.apply({
    type: "advanced",
    graph: context.activeGraph(),
    set: integer(data, "set"),
    ...(other === undefined ? {} : { other }),
    operation,
  });
}

function correlation(context: Context): void {
  const source = selectedSet(context, "Correlation and covariance");
  if (!source) return;
  showDialog(
    "Correlation and covariance",
    [
      field("Input set", source),
      field("Maximum lag", input("maximumLag", "20", "number")),
      checkbox("covariance", "Subtract each mean"),
      checkbox("unbiased", "Use the unbiased estimate"),
    ],
    (data) =>
      advanced(context, data, {
        method: "correlation",
        maximumLag: integer(data, "maximumLag", 1),
        covariance: data.has("covariance"),
        unbiased: data.has("unbiased"),
      }),
  );
}

function paired(
  context: Context,
  title: string,
  method: "digitalFilter" | "convolution",
  prompt: string,
): void {
  const source = selectedSet(context, title);
  if (!source) return;
  const other = selectedSet(context, title, "other");
  if (!other) return;
  showDialog(title, [field("Input set", source), field(prompt, other)], (data) =>
    advanced(context, data, { method }, integer(data, "other")),
  );
}

function geometric(context: Context): void {
  const source = selectedSet(context, "Geometric transforms");
  if (!source) return;
  showDialog(
    "Geometric transforms",
    [
      field("Input set", source),
      field("Rotation, degrees", input("angle", "0", "number")),
      field("Origin X", input("originX", "0", "number")),
      field("Origin Y", input("originY", "0", "number")),
      field("Scale X", input("scaleX", "1", "number")),
      field("Scale Y", input("scaleY", "1", "number")),
      field("Translate X", input("translationX", "0", "number")),
      field("Translate Y", input("translationY", "0", "number")),
      field(
        "Order",
        select(
          "order",
          [
            ["rotateTranslateScale", "Rotate, translate, scale"],
            ["rotateScaleTranslate", "Rotate, scale, translate"],
            ["translateScaleRotate", "Translate, scale, rotate"],
            ["translateRotateScale", "Translate, rotate, scale"],
            ["scaleTranslateRotate", "Scale, translate, rotate"],
            ["scaleRotateTranslate", "Scale, rotate, translate"],
          ],
          "rotateTranslateScale",
        ),
      ),
    ],
    (data) =>
      advanced(context, data, {
        method: "geometric",
        angle: number(data, "angle"),
        origin: [number(data, "originX"), number(data, "originY")],
        scale: [number(data, "scaleX"), number(data, "scaleY")],
        translation: [number(data, "translationX"), number(data, "translationY")],
        order: formText(data, "order") as GeometricOrder,
      }),
  );
}

function sampling(context: Context): void {
  const source = selectedSet(context, "Sample points");
  if (!source) return;
  showDialog(
    "Sample points",
    [
      field("Input set", source),
      field("Start point", input("start", "0", "number")),
      field("Keep every", input("step", "2", "number")),
      field("Condition", input("condition", "")),
      paragraph("An optional condition such as y > 0 selects points by expression."),
    ],
    (data) =>
      advanced(context, data, {
        method: "sampling",
        start: integer(data, "start"),
        step: integer(data, "step", 1),
        ...(formText(data, "condition").trim()
          ? { condition: formText(data, "condition").trim() }
          : {}),
      }),
  );
}

function pruning(context: Context): void {
  const source = selectedSet(context, "Prune data");
  if (!source) return;
  showDialog(
    "Prune data",
    [
      field("Input set", source),
      field(
        "Type",
        select(
          "kind",
          [
            ["interpolation", "Interpolation"],
            ["circle", "Circle"],
            ["ellipse", "Ellipse"],
            ["rectangle", "Rectangle"],
          ],
          "interpolation",
        ),
      ),
      field("Distance tolerance", input("tolerance", "0.01", "number")),
      field("X tolerance", input("deltaX", "")),
      field("Y tolerance", input("deltaY", "")),
    ],
    (data) =>
      advanced(context, data, {
        method: "pruning",
        tolerance: number(data, "tolerance"),
        kind: formText(data, "kind") as NonNullable<
          Extract<AdvancedOperation, { method: "pruning" }>["kind"]
        >,
        ...(formText(data, "deltaX").trim() ? { deltaX: number(data, "deltaX") } : {}),
        ...(formText(data, "deltaY").trim() ? { deltaY: number(data, "deltaY") } : {}),
      }),
  );
}

function nonlinearFit(context: Context): void {
  const source = selectedSet(context, "Non-linear curve fitting");
  if (!source) return;
  showDialog(
    "Non-linear curve fitting",
    [
      field("Input set", source),
      paragraph(
        "Use x and a0, a1, ... in the formula. Separate initial parameter values with commas.",
      ),
      field("Formula", input("formula", "a0 * exp(a1 * x)")),
      field("Initial parameters", input("parameters", "1, -1")),
      field("Lower bounds", input("lower", "")),
      field("Upper bounds", input("upper", "")),
      field("Iterations", input("iterations", "100", "number")),
      field("Tolerance", input("tolerance", "1e-8", "number")),
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
    (data) => {
      const parameters = formText(data, "parameters")
        .split(",")
        .map((value) => Number(value.trim()));
      if (!parameters.length || parameters.some((value) => !Number.isFinite(value)))
        throw new Error("Enter comma-separated finite initial parameter values.");
      const bounds = (name: string): number[] | undefined => {
        const source = formText(data, name).trim();
        if (!source) return undefined;
        const values = source.split(",").map((value) => Number(value.trim()));
        if (values.length !== parameters.length || values.some((value) => !Number.isFinite(value)))
          throw new Error(`${name} needs one finite value for each parameter.`);
        return values;
      };
      const lower = bounds("lower");
      const upper = bounds("upper");
      if (lower?.some((value, index) => value > (upper?.[index] ?? Infinity)))
        throw new Error("Each lower bound must be less than or equal to its upper bound.");
      const tolerance = number(data, "tolerance");
      if (tolerance <= 0) throw new Error("Tolerance must be greater than zero.");
      return context.apply({
        type: "fit",
        graph: context.activeGraph(),
        set: integer(data, "set"),
        options: {
          formula: formText(data, "formula"),
          parameters,
          ...(lower ? { lower } : {}),
          ...(upper ? { upper } : {}),
          iterations: integer(data, "iterations", 1),
          tolerance,
          output: data.get("output") === "residuals" ? "residuals" : "fitted",
        },
      });
    },
  );
}

function featureExtraction(context: Context): void {
  const graph = context.graph();
  if (!graph || !Object.values(graph.sets).some((set) => set.count > 0)) {
    showDialog("Feature extraction", [paragraph("Create or import a data set first.")]);
    return;
  }
  showDialog(
    "Feature extraction",
    [
      field(
        "Feature",
        select(
          "feature",
          [
            ["yMinimum", "Minimum Y"],
            ["yMaximum", "Maximum Y"],
            ["yAverage", "Mean Y"],
            ["yStandardDeviation", "Standard deviation of Y"],
            ["yMedian", "Median Y"],
            ["xMinimum", "Minimum X"],
            ["xMaximum", "Maximum X"],
            ["xAverage", "Mean X"],
            ["xStandardDeviation", "Standard deviation of X"],
            ["xMedian", "Median X"],
            ["frequency", "Frequency"],
            ["period", "Period"],
            ["riseTime", "Rise time"],
            ["fallTime", "Fall time"],
            ["slope", "Slope"],
            ["yIntercept", "Y intercept"],
            ["setLength", "Set length"],
            ["barycenterX", "X barycenter"],
            ["barycenterY", "Y barycenter"],
            ["xAtYMaximum", "X at maximum Y"],
            ["yAtXMaximum", "Y at maximum X"],
            ["integral", "Area under curve"],
            ["zeroCrossing", "Zero crossing"],
            ["halfMaximumWidth", "Half-maximum width"],
          ],
          "yMinimum",
        ),
      ),
      paragraph("Creates a new set with one value for each visible data set."),
    ],
    (data) =>
      context.apply({
        type: "featureExtraction",
        graph: context.activeGraph(),
        feature: formText(data, "feature") as Feature,
      }),
  );
}

export function advancedDialogs(context: Context): Record<string, () => void> {
  return {
    correlation: () => correlation(context),
    "digital-filter": () => paired(context, "Digital filter", "digitalFilter", "Filter weights"),
    convolution: () => paired(context, "Linear convolution", "convolution", "Other input set"),
    geometric: () => geometric(context),
    sampling: () => sampling(context),
    pruning: () => pruning(context),
    "nonlinear-fit": () => nonlinearFit(context),
    "feature-extraction": () => featureExtraction(context),
  };
}

import { element, field, input, paragraph, showDialog } from "./dialog";

/** Browser-local settings that affect the Grace workspace, not its project files. */
export type Preferences = {
  toolbarVisible: boolean;
  statusVisible: boolean;
  locatorVisible: boolean;
  /** Fraction of the current world moved by each scroll action, expressed as a percentage. */
  scrollPercent: number;
  /** Fraction of the current world added or removed by each zoom action, expressed as a percentage. */
  zoomPercent: number;
  /** Significant digits shown for each locator coordinate. */
  locatorPrecision: number;
};

const storageKey = "web-grace.preferences.v1";

// Grace initializes both scrollper and shexper to 0.05, and locator precision to 6.
const defaults: Preferences = {
  toolbarVisible: true,
  statusVisible: true,
  locatorVisible: true,
  scrollPercent: 5,
  zoomPercent: 5,
  locatorPrecision: 6,
};

function finitePercent(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 200;
}

function precision(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 15;
}

function zoomPercent(value: unknown): value is number {
  return finitePercent(value) && value < 50;
}

function stored(): Preferences {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "null");
    if (!value || typeof value !== "object") return { ...defaults };
    const record = value as Partial<Preferences>;
    return {
      toolbarVisible:
        typeof record.toolbarVisible === "boolean"
          ? record.toolbarVisible
          : defaults.toolbarVisible,
      statusVisible:
        typeof record.statusVisible === "boolean" ? record.statusVisible : defaults.statusVisible,
      locatorVisible:
        typeof record.locatorVisible === "boolean"
          ? record.locatorVisible
          : defaults.locatorVisible,
      scrollPercent: finitePercent(record.scrollPercent)
        ? record.scrollPercent
        : defaults.scrollPercent,
      zoomPercent: zoomPercent(record.zoomPercent) ? record.zoomPercent : defaults.zoomPercent,
      locatorPrecision: precision(record.locatorPrecision)
        ? record.locatorPrecision
        : defaults.locatorPrecision,
    };
  } catch {
    // Privacy settings and embedded browsers can deny Web Storage. Keep a usable session state.
    return { ...defaults };
  }
}

function checkbox(name: string, label: string, checked: boolean): HTMLLabelElement {
  const control = document.createElement("input");
  control.type = "checkbox";
  control.name = name;
  control.checked = checked;
  return field(label, control);
}

function number(data: FormData, name: string, valid: (value: number) => boolean): number {
  const value = Number(data.get(name));
  if (!valid(value))
    throw new Error(`Enter a valid ${name.replace(/([A-Z])/g, " $1").toLowerCase()}.`);
  return value;
}

/**
 * Owns browser-only display and navigation preferences. Project files deliberately do not
 * contain these values, matching Grace's application preferences rather than plot state.
 */
export function preferences(): {
  values: () => Preferences;
  dialog: () => void;
  apply: () => void;
  toggle: (part: "toolbar" | "status" | "locator") => void;
} {
  let values = stored();

  function apply(): void {
    element<HTMLElement>("toolbar").hidden = !values.toolbarVisible;
    element<HTMLElement>("status-bar").hidden = !values.statusVisible;
    element<HTMLElement>("locator-bar").hidden = !values.locatorVisible;
    for (const part of ["toolbar", "status", "locator"] as const)
      document
        .querySelector(`button[data-action="${part}"]`)
        ?.setAttribute("aria-pressed", String(values[`${part}Visible`]));
  }

  function save(): void {
    try {
      localStorage.setItem(storageKey, JSON.stringify(values));
    } catch {
      // The in-memory value remains authoritative for this page session.
    }
  }

  function dialog(): void {
    const scroll = input("scrollPercent", String(values.scrollPercent), "number");
    scroll.min = "0";
    scroll.max = "200";
    const zoom = input("zoomPercent", String(values.zoomPercent), "number");
    zoom.min = "0";
    zoom.max = "49.99";
    const locator = input("locatorPrecision", String(values.locatorPrecision), "number");
    locator.min = "1";
    locator.max = "15";
    locator.step = "1";

    showDialog(
      "Preferences",
      [
        paragraph("Workspace visibility"),
        checkbox("toolbarVisible", "Show tool bar", values.toolbarVisible),
        checkbox("statusVisible", "Show status bar", values.statusVisible),
        checkbox("locatorVisible", "Show locator bar", values.locatorVisible),
        paragraph("Navigation"),
        field("Scroll %", scroll),
        field("Zoom %", zoom),
        field("Locator precision", locator),
      ],
      (data) => {
        values = {
          toolbarVisible: data.has("toolbarVisible"),
          statusVisible: data.has("statusVisible"),
          locatorVisible: data.has("locatorVisible"),
          scrollPercent: number(data, "scrollPercent", finitePercent),
          zoomPercent: number(data, "zoomPercent", zoomPercent),
          locatorPrecision: number(data, "locatorPrecision", precision),
        };
        save();
        apply();
      },
    );
  }

  function toggle(part: "toolbar" | "status" | "locator"): void {
    const key = `${part}Visible` as const;
    values[key] = !values[key];
    save();
    apply();
  }

  return { values: () => ({ ...values }), dialog, apply, toggle };
}

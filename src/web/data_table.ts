import { element, field, formText, input, paragraph, select, showDialog } from "./dialog";
import { engine } from "./engine";
import { isSetKind, setColumns } from "./set_types";
import type { Command, Graph, SetKind } from "./protocol";

const pageSize = 100;
const maximumNewRows = 100_000;

type Context = {
  graph: number;
  set: number | null;
  data: Pick<Graph["sets"][string], "kind" | "count">;
  apply: (command: Command) => Promise<void>;
};

type NewContext = Pick<Context, "graph" | "apply">;
type Addition = { id: number; values: number[] };

function value(text: string): number {
  const parsed = Number(text);
  if (!text.trim() || !Number.isFinite(parsed))
    throw new Error("Spreadsheet values must be finite numbers.");
  return parsed;
}

function reportError(failure: unknown, fallback: string): void {
  const message = element("dialog-error");
  message.textContent = failure instanceof Error ? failure.message : fallback;
  message.hidden = false;
}

/** Opens the set manager's Create new > In spreadsheet creation dialog. */
export function newSpreadsheetDialog(context: NewContext): void {
  const kinds = Object.keys(setColumns) as SetKind[];
  const kind = select(
    "kind",
    kinds.map((candidate) => [candidate, candidate.toUpperCase()]),
    "xy",
  );
  const rows = input("rows", "1", "number");
  rows.min = "1";
  rows.max = String(maximumNewRows);
  rows.step = "1";
  showDialog(
    "Create data set in spreadsheet",
    [field("Set type", kind), field("Rows", rows)],
    async (form) => {
      const selected = formText(form, "kind");
      if (!isSetKind(selected)) throw new Error("Choose a supported set type.");
      const count = Number(form.get("rows"));
      if (!Number.isInteger(count) || count < 1 || count > maximumNewRows)
        throw new Error(`Rows must be an integer from 1 to ${maximumNewRows}.`);
      await spreadsheetDialog({
        ...context,
        set: null,
        data: { kind: selected, count },
      });
    },
  );
}

export async function spreadsheetDialog(context: Context): Promise<void> {
  const kind = context.data.kind;
  if (!isSetKind(kind)) {
    showDialog("Spreadsheet data set editor", [paragraph("This set type cannot be edited.")]);
    return;
  }
  const headings = setColumns[kind];
  const edits = new Map<number, number[]>();
  const deleted = new Set<number>();
  const selected = new Set<string>();
  const additions: Addition[] = [];
  const originalPages = Math.ceil(context.data.count / pageSize);
  let nextAdditionId = 0;
  let page = 0;
  let revision: number | undefined;

  async function loadPage(offset: number): Promise<number[][]> {
    if (context.set === null)
      return Array.from({ length: Math.min(pageSize, context.data.count - offset) }, () =>
        headings.map(() => 0),
      );
    const response = await engine({
      type: "dataTable",
      graph: context.graph,
      set: context.set,
      offset,
      limit: pageSize,
    });
    if (response.type !== "dataTable") throw new Error("Grace did not return data rows.");
    revision ??= response.revision;
    if (revision !== response.revision)
      throw new Error(
        "The project changed while the spreadsheet was open. Reopen the set to edit it.",
      );
    return response.rows;
  }

  function rowKey(row: HTMLTableRowElement): string {
    const key = row.dataset["row"];
    if (!key) throw new Error("Spreadsheet row has no identity.");
    return key;
  }

  function capture(): void {
    for (const row of element("dialog-content").querySelectorAll<HTMLTableRowElement>("tbody tr")) {
      const key = rowKey(row);
      const values = Array.from(row.querySelectorAll<HTMLInputElement>("input[type=number]")).map(
        (cell) => value(cell.value),
      );
      if (key.startsWith("original:")) edits.set(Number(key.slice("original:".length)), values);
      else {
        const addition = additions.find(({ id }) => key === `addition:${id}`);
        if (!addition) throw new Error("Spreadsheet row is no longer available.");
        addition.values = values;
      }
    }
  }

  function totalPages(): number {
    return originalPages + Math.ceil(additions.length / pageSize);
  }

  function rowControls(
    body: HTMLTableSectionElement,
    key: string,
    label: string,
    coordinates: readonly number[],
  ): void {
    const tr = document.createElement("tr");
    tr.dataset["row"] = key;
    const choose = document.createElement("td");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = selected.has(key);
    checkbox.setAttribute("aria-label", `Select ${label}`);
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) selected.add(key);
      else selected.delete(key);
    });
    choose.append(checkbox);
    tr.append(choose);
    const number = document.createElement("th");
    number.scope = "row";
    number.textContent = label;
    tr.append(number);
    for (const [column, coordinate] of coordinates.entries()) {
      const cell = document.createElement("td");
      const control = document.createElement("input");
      control.type = "number";
      control.step = "any";
      control.value = String(coordinate);
      control.setAttribute("aria-label", `${label}, ${headings[column] ?? column}`);
      cell.append(control);
      tr.append(cell);
    }
    body.append(tr);
  }

  const render = async (): Promise<void> => {
    const table = document.createElement("table");
    table.className = "data-table";
    const header = document.createElement("tr");
    const selectionHeading = document.createElement("th");
    selectionHeading.scope = "col";
    selectionHeading.textContent = "Select";
    header.append(selectionHeading);
    const rowHeading = document.createElement("th");
    rowHeading.scope = "col";
    rowHeading.textContent = "Row";
    header.append(rowHeading);
    for (const name of headings) {
      const cell = document.createElement("th");
      cell.scope = "col";
      cell.textContent = name;
      header.append(cell);
    }
    const head = document.createElement("thead");
    head.append(header);
    table.append(head);
    const body = document.createElement("tbody");
    if (page < originalPages) {
      const offset = page * pageSize;
      const shown = await loadPage(offset);
      shown.forEach((row, rowOffset) => {
        const index = offset + rowOffset;
        if (!deleted.has(index))
          rowControls(body, `original:${index}`, `Row ${index}`, edits.get(index) ?? row);
      });
    } else {
      const start = (page - originalPages) * pageSize;
      additions.slice(start, start + pageSize).forEach((addition) => {
        rowControls(body, `addition:${addition.id}`, "New row", addition.values);
      });
    }
    table.append(body);

    const add = document.createElement("button");
    add.type = "button";
    add.textContent = "Add row";
    add.addEventListener("click", () => {
      try {
        capture();
        additions.push({ id: nextAdditionId++, values: Array(headings.length).fill(0) });
        page = originalPages + Math.floor((additions.length - 1) / pageSize);
        void changePage(0);
      } catch (failure) {
        reportError(failure, "Could not add a spreadsheet row.");
      }
    });
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "Delete selected row";
    remove.addEventListener("click", () => {
      try {
        capture();
        if (!selected.size) throw new Error("Select at least one row to delete.");
        for (const key of selected) {
          if (key.startsWith("original:")) {
            const index = Number(key.slice("original:".length));
            deleted.add(index);
            edits.delete(index);
          } else {
            const id = Number(key.slice("addition:".length));
            const index = additions.findIndex((addition) => addition.id === id);
            if (index >= 0) additions.splice(index, 1);
          }
        }
        selected.clear();
        page = Math.min(page, Math.max(0, totalPages() - 1));
        void changePage(0);
      } catch (failure) {
        reportError(failure, "Could not delete the selected spreadsheet rows.");
      }
    });
    const previous = document.createElement("button");
    previous.type = "button";
    previous.textContent = "Previous";
    previous.disabled = page === 0;
    previous.addEventListener("click", () => void changePage(-1));
    const next = document.createElement("button");
    next.type = "button";
    next.textContent = "Next";
    next.disabled = page >= totalPages() - 1;
    next.addEventListener("click", () => void changePage(1));
    const pendingCount = context.data.count - deleted.size + additions.length;
    showDialog(
      "Spreadsheet data set editor",
      [
        paragraph(
          context.set === null
            ? `New set in G${context.graph} has ${pendingCount} points.`
            : `G${context.graph}.S${context.set} has ${pendingCount} points.`,
        ),
        table,
        add,
        remove,
        previous,
        next,
      ],
      async () => {
        capture();
        const rows: number[][] = [];
        for (let offset = 0; offset < context.data.count; offset += pageSize) {
          const loaded = await loadPage(offset);
          loaded.forEach((row, rowOffset) => {
            const index = offset + rowOffset;
            if (!deleted.has(index)) rows.push(edits.get(index) ?? row);
          });
        }
        rows.push(...additions.map((addition) => addition.values));
        await context.apply({
          type: "replaceData",
          graph: context.graph,
          set: context.set,
          kind,
          rows,
          ...(revision === undefined ? {} : { expectedRevision: revision }),
        });
        if (revision !== undefined) revision++;
      },
    );
  };

  async function changePage(delta: number): Promise<void> {
    const message = element("dialog-error");
    message.hidden = true;
    try {
      if (delta) capture();
      page = Math.max(0, Math.min(page + delta, Math.max(0, totalPages() - 1)));
      await render();
    } catch (failure) {
      reportError(failure, "Could not open the spreadsheet.");
    }
  }

  await changePage(0);
}

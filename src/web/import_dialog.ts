import { field, formText, input, paragraph, select, showDialog } from "./dialog";
import { isSetKind, setColumns } from "./set_types";
import type { Request } from "./protocol";

type ImportMapped = Extract<Request, { type: "importMapped" }>;

type Context = {
  graph: number;
  import: (request: ImportMapped) => Promise<void>;
};

function split(line: string, delimiter: string): string[] {
  if (delimiter === "comma") return line.split(",");
  if (delimiter === "tab") return line.split("\t");
  return line.trim().split(/\s+/);
}

function detectedDelimiter(line: string): "comma" | "tab" | "whitespace" {
  if (line.includes(",")) return "comma";
  if (line.includes("\t")) return "tab";
  return "whitespace";
}

function index(value: FormDataEntryValue | null, name: string): number | null {
  const text = typeof value === "string" ? value.trim() : "";
  if (!text) return null;
  const result = Number(text);
  if (!Number.isInteger(result) || result < 0)
    throw new Error(`${name} must be a zero-based column number.`);
  return result;
}

export async function importTableDialog(file: File, context: Context): Promise<void> {
  const preview = await file.slice(0, 12_000).text();
  const line =
    preview.split(/\r?\n/).find((row) => row.trim() && !row.trim().startsWith("#")) ?? "";
  const delimiter = detectedDelimiter(line);
  const columnCount = split(line, delimiter).length;
  const kind = "xy";
  const mappingControls = Array.from({ length: 6 }, (_, column) =>
    field(
      `Column for ${["X", "Y", "Y1", "Y2", "Y3", "Y4"][column]}`,
      input(`column-${column}`, column < columnCount ? String(column) : ""),
    ),
  );
  showDialog(
    "Import table",
    [
      paragraph(`${file.name} appears to have ${columnCount} columns in its first data row.`),
      field(
        "Delimiter",
        select(
          "delimiter",
          [
            ["comma", "Comma"],
            ["tab", "Tab"],
            ["whitespace", "Whitespace"],
          ],
          delimiter,
        ),
      ),
      field(
        "Set type",
        select(
          "kind",
          Object.keys(setColumns).map((id) => [id, id.toUpperCase()]),
          kind,
        ),
      ),
      ((): HTMLLabelElement => {
        const header = document.createElement("input");
        header.name = "header";
        header.type = "checkbox";
        const label = document.createElement("label");
        label.append(header, "First row is a header");
        return label;
      })(),
      ...mappingControls,
      field("Additional Y columns (NXY)", input("extraY", "")),
      field("Read X as ISO date", input("dateX", "on", "checkbox")),
      paragraph("Columns are numbered from zero. Leave X blank to use the row index."),
    ],
    async (data) => {
      const setKind = formText(data, "kind");
      if (!isSetKind(setKind)) throw new Error("Select a set type.");
      const names = setColumns[setKind];
      const columns = names.map((name, position) => index(data.get(`column-${position}`), name));
      const extra = formText(data, "extraY").trim();
      const extraY = extra
        ? extra.split(",").map((value) => {
            const column = index(value, "Additional Y column");
            if (column === null)
              throw new Error("Enter comma-separated additional Y column numbers.");
            return column;
          })
        : undefined;
      if (columns.slice(1).some((column) => column === null))
        throw new Error("Map every Y column for this set type.");
      return context.import({
        type: "importMapped",
        graph: context.graph,
        bytes: await file.arrayBuffer(),
        options: {
          delimiter: formText(data, "delimiter") as "comma" | "tab" | "whitespace",
          header: data.has("header"),
          kind: setKind,
          columns,
          ...(extraY ? { extraY } : {}),
          dateX: data.has("dateX"),
        },
      });
    },
  );
}

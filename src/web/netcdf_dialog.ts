import { field, formText, select, showDialog } from "./dialog";
import { engine } from "./engine";
import type { Request } from "./protocol";

export async function netcdfDialog(
  file: File,
  graph: number,
  apply: (request: Request) => Promise<void>,
): Promise<void> {
  const preview = await engine({ type: "netcdfVariables", bytes: await file.arrayBuffer() });
  if (preview.type !== "netcdfVariables" || !preview.variables.length)
    throw new Error("The file contains no one-dimensional numeric variables.");
  const options = preview.variables.map(([name, length]): [string, string] => [
    name,
    `${name} (${length} points)`,
  ]);
  showDialog(
    "Read NetCDF",
    [
      field("X variable", select("x", [["", "Point index"], ...options], "")),
      field("Y variable", select("y", options, options[0]?.[0] ?? "")),
    ],
    async (form) => {
      await apply({
        type: "importNetcdf",
        graph,
        bytes: await file.arrayBuffer(),
        x: formText(form, "x"),
        y: formText(form, "y"),
      });
    },
  );
}

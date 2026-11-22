import type { SetKind } from "./protocol";

export const setColumns: Record<SetKind, readonly string[]> = {
  xy: ["X", "Y"],
  bar: ["X", "Y"],
  bardy: ["X", "Y", "dY"],
  bardydy: ["X", "Y", "dY-", "dY+"],
  xydx: ["X", "Y", "dX"],
  xydy: ["X", "Y", "dY"],
  xydxdx: ["X", "Y", "dX-", "dX+"],
  xydydy: ["X", "Y", "dY-", "dY+"],
  xydxdy: ["X", "Y", "dX", "dY"],
  xydxdxdydy: ["X", "Y", "dX-", "dX+", "dY-", "dY+"],
  xyhilo: ["X", "High", "Low", "Open", "Close"],
  xyr: ["X", "Y", "Radius"],
  xysize: ["X", "Y", "Size"],
  xycolor: ["X", "Y", "Color"],
  xycolpat: ["X", "Y", "Color", "Pattern"],
  xyvmap: ["X", "Y", "VX", "VY"],
  xyboxplot: ["X", "Median", "Box low", "Box high", "Whisker low", "Whisker high"],
  xyz: ["X", "Y", "Z"],
};

export function isSetKind(kind: string): kind is SetKind {
  return Object.prototype.hasOwnProperty.call(setColumns, kind);
}

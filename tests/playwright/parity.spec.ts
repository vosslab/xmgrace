import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";

import { expect, test } from "@playwright/test";

async function downloadFigure(
  page: import("@playwright/test").Page,
  testInfo: import("@playwright/test").TestInfo,
  format: "PNG" | "PDF",
): Promise<Buffer> {
  await page.getByText("Data", { exact: true }).click();
  await page.getByText("Export", { exact: true }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: `${format}...` }).click();
  const dialog = page.getByRole("dialog");
  if (format === "PNG") await dialog.getByLabel("Width (pixels)", { exact: true }).fill("640");
  await dialog.getByRole("button", { name: "Accept" }).click();
  const path = testInfo.outputPath(`figure.${format.toLowerCase()}`);
  await (await download).saveAs(path);
  return readFile(path);
}

test("PNG and PDF exports are genuine files at the requested page dimensions", async ({
  page,
}, testInfo) => {
  await page.goto("./");

  const png = await downloadFigure(page, testInfo, "PNG");
  expect(png.subarray(0, 8)).toEqual(Buffer.from("89504e470d0a1a0a", "hex"));
  expect(png.readUInt32BE(16)).toBe(640);
  expect(png.readUInt32BE(20)).toBeGreaterThan(0);

  const pdf = await downloadFigure(page, testInfo, "PDF");
  expect(pdf.subarray(0, 5).toString("ascii")).toBe("%PDF-");
  const text = pdf.toString("latin1");
  const mediaBox = text.match(/\/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]/);
  expect(mediaBox).not.toBeNull();
  expect(Number(mediaBox?.[1])).toBeGreaterThan(0);
  expect(Number(mediaBox?.[2])).toBeGreaterThan(0);
});

test("drawing-object manager keeps the configuration dialog open for a new object", async ({
  page,
}) => {
  await page.goto("./");
  await page.getByText("Window", { exact: true }).click();
  await page.getByRole("button", { name: "Drawing objects" }).click();
  const manager = page.getByRole("dialog");
  await manager.getByRole("combobox", { name: "Action", exact: true }).selectOption("create");
  await manager
    .getByRole("combobox", { name: "New object type", exact: true })
    .selectOption("line");
  await manager.getByRole("button", { name: "Accept" }).click();
  const editor = page.getByRole("dialog");
  await expect(editor).toHaveAccessibleName("Create drawing object");
  await expect(editor.getByRole("combobox", { name: "Type", exact: true })).toHaveValue("line");
  await expect(editor.getByRole("button", { name: "Accept" })).toBeVisible();
});

test("classic controls open a spreadsheet that creates a set on Accept", async ({ page }) => {
  await page.goto("./");
  const toolbar = page.getByRole("complementary", { name: "Grace toolbar" });
  for (const name of [
    "Draw",
    "Z",
    "AS",
    "AutoT",
    "AutoO",
    "ZX",
    "ZY",
    "AX",
    "AY",
    "PZ",
    "Pu",
    "Po",
    "Cy",
    "Exit",
  ])
    await expect(toolbar.getByRole("button", { name, exact: true })).toBeVisible();

  await page.getByText("Edit", { exact: true }).click();
  await page.getByRole("button", { name: "Data sets..." }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText("Create new", { exact: true }).click();
  await dialog.getByRole("button", { name: "In spreadsheet..." }).click();
  await dialog.getByRole("button", { name: "Accept" }).click();
  await expect(dialog).toHaveAccessibleName("Spreadsheet data set editor");
  await dialog.getByRole("spinbutton", { name: "Row 0, X" }).fill("2");
  await dialog.getByRole("spinbutton", { name: "Row 0, Y" }).fill("4");
  await expect(page.locator("#status-bar")).toContainText("0 sets");
  await dialog.getByRole("button", { name: "Accept" }).click();
  await expect(page.locator("#status-bar")).toContainText("1 set");
});

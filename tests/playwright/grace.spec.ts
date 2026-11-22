import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";

import { expect, test } from "@playwright/test";

test("import XY data, edit axis properties, and reopen the saved Grace project", async ({
  page,
}, testInfo) => {
  await page.goto("./");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
  await page
    .getByRole("img", { name: "Grace project plot" })
    .evaluate((image: HTMLImageElement) => image.decode());
  await page.locator("#data-file").setInputFiles({
    name: "observations.dat",
    mimeType: "text/plain",
    buffer: Buffer.from("0 1\n1 2\n2 4\n"),
  });
  await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");
  await page.getByText("Plot", { exact: true }).click();
  await page.getByRole("button", { name: "Axis properties..." }).click();
  await page.getByLabel("Label", { exact: true }).fill("Elapsed time");
  await page.getByRole("button", { name: "Accept" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.getByText("File", { exact: true }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  const file = testInfo.outputPath("roundtrip.agr");
  await (await download).saveAs(file);
  await page.locator("#project-file").setInputFiles(file);
  await expect(page.locator("#project-title")).toHaveText("Grace: roundtrip.agr");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");
  await page.getByText("Plot", { exact: true }).click();
  await page.getByRole("button", { name: "Axis properties..." }).click();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue("Elapsed time");
});

test("unsupported Grace projects retain the original and disable export", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
  await page.locator("#project-file").setInputFiles({
    name: "future.agr",
    mimeType: "text/plain",
    buffer: Buffer.from("@version 50125\n@g0 type UnsupportedGraphType\n@with g0\n"),
  });
  await expect(page.getByText("Read-only preview.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Download original" })).toBeVisible();
  await page.getByText("File", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "View diagnostics..." }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "Line 2: Unsupported Grace setting: g0 type UnsupportedGraphType",
  );
});

test("classic toolbar labels and formula FFT workflow survive an AGR round trip", async ({
  page,
}, testInfo) => {
  await page.goto("./");
  const toolbar = page.getByLabel("Grace toolbar");
  await expect(toolbar).toContainText("Draw");
  await expect(toolbar).toContainText("Z");
  await expect(toolbar).toContainText("AS");
  await expect(toolbar).toContainText("AX");
  await expect(toolbar).toContainText("AY");

  await page.getByText("Edit", { exact: true }).click();
  await page.getByRole("button", { name: "Data sets..." }).click();
  await page.getByText("Create new", { exact: true }).click();
  await page.getByRole("button", { name: "By formula..." }).click();
  const formula = page.getByRole("dialog");
  await formula.getByLabel("Stop", { exact: true }).fill("2*pi*(127/128)");
  await formula.getByLabel("Points", { exact: true }).fill("128");
  await formula.getByLabel("Y", { exact: true }).fill("sin($t)");
  await formula.getByRole("button", { name: "Accept" }).click();
  await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");

  await page.getByText("Data", { exact: true }).click();
  await page.getByText("Transformations", { exact: true }).click();
  await page.getByRole("button", { name: "Fourier transforms..." }).click();
  const transform = page.getByRole("dialog");
  await transform.getByRole("combobox", { name: "Algorithm", exact: true }).selectOption("fft");
  await transform.getByRole("combobox", { name: "Load", exact: true }).selectOption("magnitude");
  await transform.getByRole("button", { name: "Accept" }).click();
  await expect(page.locator("#status-bar")).toHaveText("G0  |  2 sets");

  await page.getByText("File", { exact: true }).click();
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  const file = testInfo.outputPath("formula-fft.agr");
  await (await download).saveAs(file);
  const agr = await readFile(file, "utf8");
  const spectrum = agr
    .split("@target G0.S1\n")[1]
    ?.split("&")[0]
    ?.trim()
    .split("\n")
    .filter((line) => !line.startsWith("@"))
    .map((line) => line.split(/\s+/).map(Number));
  expect(spectrum).toHaveLength(64);
  const magnitudeAt = (index: number): number => spectrum?.[index]?.[1] ?? Number.NaN;
  expect(spectrum?.[1]?.[0]).toBe(1);
  expect(magnitudeAt(1)).toBeCloseTo(128, 9);
  expect(magnitudeAt(1)).toBeGreaterThan(magnitudeAt(0));
  expect(magnitudeAt(1)).toBeGreaterThan(magnitudeAt(2));
  await page.locator("#project-file").setInputFiles(file);
  await expect(page.locator("#status-bar")).toHaveText("G0  |  2 sets");
});

test("the built-in examples open as editable projects", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
  for (const filename of ["cooling.agr", "measurement_errors.agr"]) {
    await page.getByText("Help", { exact: true }).click();
    await page.getByRole("button", { name: "Examples", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("combobox").selectOption(filename);
    await dialog.getByRole("button", { name: "Accept" }).click();
    await expect(page.locator("#project-title")).toHaveText(`Grace: ${filename}`);
    await expect(page.getByText("Read-only preview.")).toBeHidden();
    await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");
  }
  await page.getByText("File", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "New", exact: true }).click();
  await expect(page.locator("#project-title")).toHaveText("Grace: Untitled.agr");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
});

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
  await expect(page.locator("#project-title")).toHaveText("Grace: Untitled.agr *");
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

test("guided demo keeps the original project intact while teaching a pausable FFT workflow", async ({
  page,
}, testInfo) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      "web-grace.preferences.v1",
      JSON.stringify({
        toolbarVisible: false,
        statusVisible: true,
        locatorVisible: true,
        scrollPercent: 5,
        zoomPercent: 5,
        locatorPrecision: 6,
      }),
    ),
  );
  await page.goto("./");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
  await expect(page.getByLabel("Grace toolbar")).toBeHidden();
  await page.locator("#data-file").setInputFiles({
    name: "original.dat",
    mimeType: "text/plain",
    buffer: Buffer.from("0 1\n1 2\n"),
  });
  await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");

  const popup = page.waitForEvent("popup");
  await page.getByText("Help", { exact: true }).click();
  await page.getByRole("button", { name: "Guided demo...", exact: true }).click();
  const demo = await popup;
  await expect(demo.locator("#status-bar")).toHaveText("G0  |  0 sets");
  const guide = demo.getByLabel("Guided demo");
  await expect(guide).toBeVisible();
  await expect(guide).toContainText("Step 1 of 3");
  await expect(guide).toContainText("Create a signal");
  await expect(demo.getByLabel("Grace toolbar")).toBeVisible();
  await demo.getByText("View", { exact: true }).click();
  await demo.getByRole("button", { name: "Show locator bar", exact: true }).click();
  await expect(demo.locator("#locator-bar")).toBeHidden();
  const normal = await demo.context().newPage();
  await normal.goto("./");
  await expect(normal.locator("#locator-bar")).toBeVisible();
  await normal.close();

  await demo.getByText("Edit", { exact: true }).click();
  await demo.getByRole("button", { name: "Data sets..." }).click();
  await demo.getByText("Create new", { exact: true }).click();
  await demo.getByRole("button", { name: "By formula..." }).click();
  const formula = demo.getByRole("dialog");
  await expect(formula).toContainText("2*pi*127/128");
  await formula.getByLabel("Stop", { exact: true }).fill("2*pi*127/128");
  await formula.getByLabel("Points", { exact: true }).fill("128");
  await formula.getByLabel("Y", { exact: true }).fill("sin(5*$t)+0.5*sin(13*$t)");
  await formula.getByRole("button", { name: "Accept", exact: true }).click();
  await expect(demo.locator("#status-bar")).toHaveText("G0  |  1 set");

  await guide.getByRole("button", { name: "Pause", exact: true }).click();
  await expect(guide.getByRole("heading")).toBeHidden();
  await expect(demo.getByRole("button", { name: "Resume", exact: true })).toBeFocused();
  await demo.getByText("Plot", { exact: true }).click();
  await demo.getByRole("button", { name: "Graph appearance..." }).click();
  await demo.getByLabel("Title", { exact: true }).fill("Changed during pause");
  await demo.getByRole("dialog").getByRole("button", { name: "Accept", exact: true }).click();
  await guide.getByRole("button", { name: "Resume", exact: true }).click();
  await expect(guide).toContainText("Create a signal");
  await expect(guide.getByRole("button", { name: "Pause", exact: true })).toBeFocused();

  await guide.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(guide).toContainText("Find its frequencies");
  await demo.getByText("Data", { exact: true }).click();
  await demo.getByText("Transformations", { exact: true }).click();
  await demo.getByRole("button", { name: "Fourier transforms..." }).click();
  const transform = demo.getByRole("dialog");
  await expect(transform).toContainText("Peaks appear at bins 5 and 13.");
  await transform.getByRole("button", { name: "Accept", exact: true }).click();
  await expect(demo.locator("#status-bar")).toHaveText("G0  |  2 sets");

  await guide.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(guide).toContainText("Save the project");
  await demo.getByText("File", { exact: true }).click();
  const download = demo.waitForEvent("download");
  await demo.getByRole("button", { name: "Save as...", exact: true }).click();
  await demo.getByLabel("File name", { exact: true }).fill("guided-demo.agr");
  await demo.getByRole("dialog").getByRole("button", { name: "Accept", exact: true }).click();
  const file = testInfo.outputPath("guided-demo.agr");
  await (await download).saveAs(file);
  const project = await readFile(file, "utf8");
  expect(project).toMatch(/^@[ \t]*title "Changed during pause"$/m);
  const spectrum = project
    .split("@target G0.S1\n")[1]
    ?.split("&")[0]
    ?.trim()
    .split("\n")
    .filter((line) => !line.startsWith("@"))
    .map((line) => line.split(/\s+/).map(Number));
  const magnitudeAt = (index: number): number => spectrum?.[index]?.[1] ?? Number.NaN;
  expect(magnitudeAt(5)).toBeGreaterThan(magnitudeAt(4));
  expect(magnitudeAt(5)).toBeGreaterThan(magnitudeAt(6));
  expect(magnitudeAt(13)).toBeGreaterThan(magnitudeAt(12));
  expect(magnitudeAt(13)).toBeGreaterThan(magnitudeAt(14));

  await guide.getByRole("button", { name: "Finish", exact: true }).click();
  await expect(guide).toBeHidden();
  await expect(demo.locator("#canvas")).toBeFocused();
  await expect(page.locator("#status-bar")).toHaveText("G0  |  1 set");
  await expect(page.locator("#project-title")).toHaveText("Grace: Untitled.agr *");
});

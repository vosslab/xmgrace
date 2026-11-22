import { Buffer } from "node:buffer";

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
    buffer: Buffer.from("@version 50125\n@g0 type Polar\n@with g0\n"),
  });
  await expect(page.getByText("Read-only preview.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Download original" })).toBeVisible();
  await page.getByText("File", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "View diagnostics..." }).click();
  await expect(page.getByRole("dialog")).toContainText(
    "Line 2: Unsupported Grace setting: g0 type Polar",
  );
});

test("the built-in examples open as editable projects", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator("#status-bar")).toHaveText("G0  |  0 sets");
  for (const filename of ["cooling.agr", "measurement_errors.agr"]) {
    await page.getByText("Help", { exact: true }).click();
    await page.getByRole("button", { name: "Examples..." }).click();
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

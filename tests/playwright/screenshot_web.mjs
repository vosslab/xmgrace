import { Buffer } from "node:buffer";

import { chromium } from "playwright";

const [url, outputPath] = process.argv.slice(2);
if (!url || !outputPath) {
  throw new Error("Usage: node tests/playwright/screenshot_web.mjs <url> <output.png>");
}

// Keep the readings in sync with the cooling plot example in README.md.
const readings = "0 85\n2 68\n4 55\n6 45\n8 38\n10 33\n12 29\n";
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(url);
  const status = page.locator("#status-bar");
  await status.filter({ hasText: /G0\s*\|\s*0 sets/ }).waitFor();

  async function openMenu(menu, item) {
    await page.getByText(menu, { exact: true }).click();
    await page.getByRole("button", { name: item, exact: true }).click();
  }

  const chooseData = page.waitForEvent("filechooser");
  await openMenu("Data", "Import ASCII...");
  await (
    await chooseData
  ).setFiles({
    name: "cooling.dat",
    mimeType: "text/plain",
    buffer: Buffer.from(readings),
  });
  await status.filter({ hasText: /G0\s*\|\s*1 set/ }).waitFor();

  const dialog = page.getByRole("dialog");
  async function accept() {
    await dialog.getByRole("button", { name: "Accept", exact: true }).click();
    await dialog.waitFor({ state: "hidden" });
  }

  await openMenu("Plot", "Axis properties...");
  await dialog.getByLabel("Label", { exact: true }).fill("Time (min)");
  await dialog.getByLabel("Start", { exact: true }).fill("-1");
  await dialog.getByLabel("Stop", { exact: true }).fill("13");
  await accept();

  await openMenu("Plot", "Axis properties...");
  await dialog.locator("select[name=axis]").selectOption("y");
  await dialog.getByLabel("Label", { exact: true }).fill("Temperature (\u00b0C)");
  await accept();

  await openMenu("Plot", "Graph appearance...");
  await dialog.getByLabel("Title", { exact: true }).fill("Cooling curve");
  await accept();

  await openMenu("Plot", "Set appearance...");
  await dialog.locator("select[name=color]").selectOption({ label: "Blue" });
  await dialog.locator("select[name=symbol]").selectOption({ label: "Circle" });
  await accept();

  await openMenu("File", "Save as...");
  await dialog.getByLabel("File name", { exact: true }).fill("cooling.agr");
  const savedProject = page.waitForEvent("download");
  await accept();
  await (await savedProject).path();
  await page
    .locator("#project-title")
    .filter({ hasText: /^Grace: cooling\.agr$/ })
    .waitFor();

  await page.getByRole("img", { name: "Grace project plot" }).evaluate((image) => image.decode());
  await page.mouse.move(1240, 785);
  await page.screenshot({ path: outputPath, fullPage: true });

  const exportedPlot = page.waitForEvent("download");
  await openMenu("Data", "Export SVG...");
  await (await exportedPlot).path();
} finally {
  await browser.close();
}

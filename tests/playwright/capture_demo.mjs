import { mkdtemp, rm, stat } from "node:fs/promises";
import { execFile } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { chromium } from "playwright";

const execute = promisify(execFile);
const frameDurations = [1100, 1600, 1650];
const [url, outputPath] = process.argv.slice(2);
if (!url || !outputPath) {
  throw new Error("Usage: node tests/playwright/capture_demo.mjs <url> <output.webp>");
}

const frameDirectory = await mkdtemp(join(tmpdir(), "grace-demo-"));
let browser;
try {
  if (frameDurations.reduce((total, duration) => total + duration, 0) > 5000)
    throw new Error("The Fourier demo exceeds the five-second animation limit.");
  browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  const status = page.locator("#status-bar");
  const guide = page.locator("#guided-demo");
  const plot = page.getByRole("img", { name: "Grace project plot" });
  const frame = async (name) => {
    await plot.evaluate((image) => image.decode());
    await page.screenshot({ path: join(frameDirectory, `${name}.png`) });
  };

  const demoUrl = new URL(url);
  demoUrl.searchParams.set("demo", "1");
  await page.goto(demoUrl.toString());
  await status.filter({ hasText: /G0\s*\|\s*0 sets/ }).waitFor();
  await guide.waitFor();
  await guide.getByRole("heading", { name: "Creating a signal", exact: true }).waitFor();
  await frame("intro");

  await status.filter({ hasText: /G0\s*\|\s*1 set/ }).waitFor();
  await guide.getByRole("heading", { name: "Two-tone signal", exact: true }).waitFor();
  await frame("waveform");

  await status.filter({ hasText: /G0\s*\|\s*2 sets/ }).waitFor();
  await guide.getByRole("heading", { name: "Fourier spectrum", exact: true }).waitFor();
  await frame("spectrum");

  await execute("img2webp", [
    "-loop",
    "1",
    "-lossy",
    "-q",
    "90",
    "-d",
    String(frameDurations[0]),
    join(frameDirectory, "intro.png"),
    "-d",
    String(frameDurations[1]),
    join(frameDirectory, "waveform.png"),
    "-d",
    String(frameDurations[2]),
    join(frameDirectory, "spectrum.png"),
    "-o",
    outputPath,
  ]);
  const { stdout } = await execute("webpmux", ["-info", outputPath]);
  if (!/Loop Count\s*:\s*1/.test(stdout) || !/Number of frames\s*:\s*3/.test(stdout))
    throw new Error(`Unexpected WebP animation metadata:\n${stdout}`);
  if ((await stat(outputPath)).size > 1_000_000)
    throw new Error("The Fourier demo exceeds the 1 MB screenshot budget.");
} finally {
  await browser?.close();
  await rm(frameDirectory, { recursive: true, force: true });
}

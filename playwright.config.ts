/// <reference types="node" />
import { defineConfig } from "@playwright/test";

const port = process.env["PW_PORT"] ?? "4173";
const origin = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "tests/playwright",
  testIgnore: ["**/_temp*", "**/dist_*/**"],
  timeout: 30_000,
  fullyParallel: true,
  reporter: "list",
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
  use: { baseURL: `${origin}/xmgrace/`, headless: true },
  webServer: {
    command: `mkdir -p generated/test-site && ln -sfn ../../dist generated/test-site/xmgrace && bash -c 'source source_me.sh && python3 -m http.server ${port} --bind 127.0.0.1 --directory generated/test-site'`,
    url: `${origin}/xmgrace/`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});

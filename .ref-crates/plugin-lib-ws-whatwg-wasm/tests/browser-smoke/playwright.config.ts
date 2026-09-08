import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  use: {
    ...devices["Desktop Chrome"],
    headless: true,
  },
  webServer: {
    command: "bun server.ts",
    url: "http://localhost:8787",
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});

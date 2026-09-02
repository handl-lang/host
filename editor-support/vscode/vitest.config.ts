import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  resolve: {
    alias: {
      vscode: path.resolve(__dirname, "src/__stubs__/vscode.js"),
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/__stubs__/setup.ts"],
    testTimeout: 30000,
    server: {
      deps: {
        inline: ["vscode-languageclient"],
      },
    },
  },
});

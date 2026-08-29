import { defineConfig } from "vite";

export default defineConfig({
  build: {
    ssr: "src/extension.ts",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    minify: false,
    target: "node18",
    rollupOptions: {
      external: ["vscode"],
      output: {
        entryFileNames: "extension.js",
        format: "cjs",
      },
    },
  },
  ssr: {
    noExternal: true,
  },
  resolve: {
    conditions: ["node"],
  },
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
});

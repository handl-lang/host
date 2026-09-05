import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { CONFIG_DEFAULTS, readConfig } from "./config";
import { DEFAULT_EDITOR_CONFIG } from "./manifest";

describe("CONFIG_DEFAULTS", () => {
  it("matches DEFAULT_EDITOR_CONFIG", () => {
    expect(CONFIG_DEFAULTS.serverPath).toBe("");
    expect(CONFIG_DEFAULTS.enableInlayHints).toBe(DEFAULT_EDITOR_CONFIG.enableInlayHints);
    expect(CONFIG_DEFAULTS.enableCodeLens).toBe(DEFAULT_EDITOR_CONFIG.enableCodeLens);
    expect(CONFIG_DEFAULTS.enableRefinementDiagnostics).toBe(
      DEFAULT_EDITOR_CONFIG.enableRefinementDiagnostics,
    );
    expect(CONFIG_DEFAULTS.enableSemanticHighlighting).toBe(
      DEFAULT_EDITOR_CONFIG.enableSemanticHighlighting,
    );
  });

  it("no longer declares contributes.configuration in package.json", async () => {
    const pkg = await import("../package.json");
    // ADR 0004: configuration settings live in .handl.toml, not package.json.
    expect(pkg.contributes.configuration).toBeUndefined();
  });
});

describe("readConfig", () => {
  function withTempDir<T>(fn: (dir: string) => T): T {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "handl-config-"));
    try {
      return fn(dir);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }

  it("returns defaults when no manifest is present", () => {
    withTempDir((dir) => {
      const config = readConfig(dir);
      expect(config).toEqual(CONFIG_DEFAULTS);
    });
  });

  it("returns defaults when workspace root is empty", () => {
    expect(readConfig("")).toEqual(CONFIG_DEFAULTS);
  });

  it("overrides defaults with manifest editor values", () => {
    withTempDir((dir) => {
      const content = ["[editor]", "enableInlayHints = false", "enableCodeLens = false"].join("\n");
      fs.writeFileSync(path.join(dir, ".handl.toml"), content);
      const config = readConfig(dir);
      expect(config.enableInlayHints).toBe(false);
      expect(config.enableCodeLens).toBe(false);
      expect(config.enableRefinementDiagnostics).toBe(CONFIG_DEFAULTS.enableRefinementDiagnostics);
      expect(config.enableSemanticHighlighting).toBe(CONFIG_DEFAULTS.enableSemanticHighlighting);
      expect(config.serverPath).toBe("");
    });
  });
});

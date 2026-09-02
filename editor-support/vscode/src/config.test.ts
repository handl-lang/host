import { describe, it, expect } from "vitest";
import { CONFIG_DEFAULTS } from "./config";

describe("CONFIG_DEFAULTS", () => {
  it("matches package.json defaults", async () => {
    const pkg = await import("../package.json");
    const props = pkg.contributes.configuration.properties;

    expect(CONFIG_DEFAULTS.serverPath).toBe(props["handl.server.path"].default);
    expect(CONFIG_DEFAULTS.enableInlayHints).toBe(props["handl.enableInlayHints"].default);
    expect(CONFIG_DEFAULTS.enableCodeLens).toBe(props["handl.enableCodeLens"].default);
    expect(CONFIG_DEFAULTS.enableRefinementDiagnostics).toBe(
      props["handl.enableRefinementDiagnostics"].default,
    );
    expect(CONFIG_DEFAULTS.enableSemanticHighlighting).toBe(
      props["handl.enableSemanticHighlighting"].default,
    );
  });
});

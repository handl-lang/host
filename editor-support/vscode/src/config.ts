import { DEFAULT_EDITOR_CONFIG, readManifest } from "./manifest";

export interface HandlConfig {
  readonly serverPath: string;
  readonly enableInlayHints: boolean;
  readonly enableCodeLens: boolean;
  readonly enableRefinementDiagnostics: boolean;
  readonly enableSemanticHighlighting: boolean;
}

/**
 * Fallback configuration used when `.handl.toml` is absent or does not
 * specify a value. Mirrors {@link DEFAULT_EDITOR_CONFIG}.
 */
export const CONFIG_DEFAULTS: HandlConfig = {
  serverPath: "",
  enableInlayHints: DEFAULT_EDITOR_CONFIG.enableInlayHints,
  enableCodeLens: DEFAULT_EDITOR_CONFIG.enableCodeLens,
  enableRefinementDiagnostics: DEFAULT_EDITOR_CONFIG.enableRefinementDiagnostics,
  enableSemanticHighlighting: DEFAULT_EDITOR_CONFIG.enableSemanticHighlighting,
};

/**
 * Reads editor configuration from `.handl.toml` in the workspace root,
 * falling back to {@link CONFIG_DEFAULTS} for any unspecified values.
 *
 * The server path is not resolved here — it is resolved by `server.ts`
 * (which checks the workspace `target/debug` directory). `serverPath` is
 * left empty on the returned config.
 */
export function readConfig(workspaceRoot: string): HandlConfig {
  const manifest = readManifest(workspaceRoot);
  const editor = manifest?.editor ?? {};
  return {
    serverPath: "",
    enableInlayHints: editor.enableInlayHints ?? CONFIG_DEFAULTS.enableInlayHints,
    enableCodeLens: editor.enableCodeLens ?? CONFIG_DEFAULTS.enableCodeLens,
    enableRefinementDiagnostics:
      editor.enableRefinementDiagnostics ?? CONFIG_DEFAULTS.enableRefinementDiagnostics,
    enableSemanticHighlighting:
      editor.enableSemanticHighlighting ?? CONFIG_DEFAULTS.enableSemanticHighlighting,
  };
}

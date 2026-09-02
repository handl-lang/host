import * as vscode from "vscode";

export interface HandlConfig {
  readonly serverPath: string;
  readonly enableInlayHints: boolean;
  readonly enableCodeLens: boolean;
  readonly enableRefinementDiagnostics: boolean;
  readonly enableSemanticHighlighting: boolean;
}

export const CONFIG_DEFAULTS: HandlConfig = {
  serverPath: "",
  enableInlayHints: true,
  enableCodeLens: true,
  enableRefinementDiagnostics: true,
  enableSemanticHighlighting: true,
};

export function readConfig(): HandlConfig {
  const c = vscode.workspace.getConfiguration("handl");
  return {
    serverPath: c.get("server.path", CONFIG_DEFAULTS.serverPath),
    enableInlayHints: c.get("enableInlayHints", CONFIG_DEFAULTS.enableInlayHints),
    enableCodeLens: c.get("enableCodeLens", CONFIG_DEFAULTS.enableCodeLens),
    enableRefinementDiagnostics: c.get("enableRefinementDiagnostics", CONFIG_DEFAULTS.enableRefinementDiagnostics),
    enableSemanticHighlighting: c.get("enableSemanticHighlighting", CONFIG_DEFAULTS.enableSemanticHighlighting),
  };
}

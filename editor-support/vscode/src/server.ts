import * as path from "path";

/**
 * Configuration shape for the `handl-lsp` extension settings.
 */
export interface ServerConfig {
  "server.path": string;
}

/**
 * A minimal workspace folder shape (mirrors `vscode.WorkspaceFolder`).
 */
export interface WorkspaceFolder {
  uri: { fsPath: string };
}

/**
 * Injectable dependencies so `resolveServerPath` can be unit-tested
 * without the `vscode` module.
 */
export interface ResolveDeps {
  getConfig: () => ServerConfig;
  getWorkspaceFolders: () => WorkspaceFolder[] | undefined;
  exists: (p: string) => boolean;
}

/**
 * Resolves the path to the `handl-host-lsp` binary.
 *
 * Priority: explicit `handl-lsp.server.path` setting, then
 * `<first-workspace>/target/debug/handl-host-lsp`.
 * Returns `null` if no candidate is found.
 */
export function resolveServerPath(deps: ResolveDeps): string | null {
  const configured = deps.getConfig()["server.path"];
  if (configured) return configured;

  const folders = deps.getWorkspaceFolders();
  if (!folders || folders.length === 0) return null;

  const candidate = path.join(folders[0].uri.fsPath, "target", "debug", "handl-host-lsp");
  if (deps.exists(candidate)) return candidate;

  return null;
}

/**
 * Options for creating the LSP client (transport-neutral).
 */
export interface ServerOptions {
  command: string;
  args: string[];
}

/**
 * Builds the server options for the language client.
 */
export function buildServerOptions(serverPath: string): ServerOptions {
  return { command: serverPath, args: [] };
}

/**
 * Document selector for `.hnl` files on the `file` scheme.
 */
export const DOCUMENT_SELECTOR = [{ scheme: "file", language: "handl" }] as const;

/**
 * The output channel name shown in VS Code's Output panel.
 */
export const OUTPUT_CHANNEL_NAME = "handl LSP";

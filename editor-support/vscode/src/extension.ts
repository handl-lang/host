import * as vscode from "vscode";
import { readConfig } from "./config";
import { createClient } from "./client";
import { registerFormatting } from "./formatting";
import { registerLanguageConfiguration } from "./language-config";
import { resolveServerPath, type ResolveDeps, type ServerConfig } from "./server";
import { readManifest } from "./manifest";

function depsFromContext(): ResolveDeps {
  return {
    getConfig: () => vscode.workspace.getConfiguration("handl") as unknown as ServerConfig,
    getWorkspaceFolders: () =>
      vscode.workspace.workspaceFolders ? [...vscode.workspace.workspaceFolders] : undefined,
    exists: (p: string) =>
      vscode.workspace.fs.stat(vscode.Uri.file(p)).then(
        () => true,
        () => false,
      ) as unknown as boolean,
  };
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * Maps additional file extensions declared in `.handl.toml` to the `handl`
 * language id via VS Code's `files.associations` setting (workspace-level).
 *
 * The `.hnl` extension is already declared statically in `package.json`; this
 * only adds any extra extensions the manifest lists (e.g. `.hn2`, `.handl`).
 */
async function registerFileAssociations(workspaceRoot: string): Promise<void> {
  const manifest = readManifest(workspaceRoot);
  const extensions = manifest?.language?.extensions;
  if (!extensions || extensions.length === 0) return;

  const filesConfig = vscode.workspace.getConfiguration("files");
  const associations = filesConfig.get<Record<string, string>>("associations", {});
  for (const ext of extensions) {
    associations[`*${ext}`] = "handl";
  }
  await filesConfig.update("associations", associations, false);
}

export async function activate(ctx: vscode.ExtensionContext): Promise<void> {
  registerLanguageConfiguration(ctx);

  const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";

  // ADR 0004: map additional file extensions from `.handl.toml` at runtime
  // via `files.associations` so the extension never needs repackaging for a
  // new extension.
  await registerFileAssociations(workspaceRoot);

  const config = readConfig(workspaceRoot);
  const serverPath = resolveServerPath(depsFromContext());

  if (!serverPath) {
    vscode.window.showWarningMessage(
      "handl: server binary not found. Build it with `cargo build -p handl-host-lsp`.",
    );
    return;
  }

  const client = createClient(serverPath, config);
  ctx.subscriptions.push(client);

  try {
    await client.start();
  } catch (err) {
    vscode.window.showErrorMessage("handl: " + errorMessage(err));
    return;
  }

  registerFormatting(ctx, client);
}

export function deactivate(): Thenable<void> | undefined {
  return undefined;
}

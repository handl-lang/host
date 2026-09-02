import * as vscode from "vscode";
import { readConfig } from "./config";
import { createClient } from "./client";
import { registerFormatting } from "./formatting";
import { registerLanguageConfiguration } from "./language-config";
import { resolveServerPath, type ResolveDeps, type ServerConfig } from "./server";

function depsFromContext(): ResolveDeps {
  return {
    getConfig: () =>
      vscode.workspace.getConfiguration("handl") as unknown as ServerConfig,
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

export async function activate(ctx: vscode.ExtensionContext): Promise<void> {
  registerLanguageConfiguration(ctx);

  const config = readConfig();
  const serverPath = resolveServerPath(depsFromContext());

  if (!serverPath) {
    vscode.window.showWarningMessage(
      "handl: server binary not found. Build it with `cargo build -p handl-host-lsp` or set `handl.server.path`.",
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

import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";
import {
  buildServerOptions,
  DOCUMENT_SELECTOR,
  OUTPUT_CHANNEL_NAME,
  resolveServerPath,
  type ResolveDeps,
} from "./server";

// Dev workflow: run `npm run dev` in editor-support/vscode to package and
// install the extension, then reload the VS Code window
// (Command Palette > Developer: Reload Window) to pick up changes.

let client: LanguageClient | null = null;

function depsFromContext(): ResolveDeps {
  return {
    getConfig: () =>
      vscode.workspace.getConfiguration("handl-lsp") as unknown as {
        "server.path": string;
      },
    getWorkspaceFolders: () =>
      vscode.workspace.workspaceFolders ? [...vscode.workspace.workspaceFolders] : undefined,
    exists: (p: string) =>
      vscode.workspace.fs.stat(vscode.Uri.file(p)).then(
        () => true,
        () => false,
      ) as unknown as boolean,
  };
}

export function activate(context: vscode.ExtensionContext): void {
  const serverPath = resolveServerPath(depsFromContext());
  if (!serverPath) {
    vscode.window.showWarningMessage(
      "handl-lsp: server binary not found. Build it with `cargo build -p handl-host-lsp` or set `handl-lsp.server.path`.",
    );
    return;
  }

  console.log("[handl-lsp] server path:", serverPath);

  try {
    const serverOptions = {
      ...buildServerOptions(serverPath),
      transport: TransportKind.stdio,
    };

    const outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
    console.log("[handl-lsp] output channel created");

    const clientOptions = {
      documentSelector: [...DOCUMENT_SELECTOR],
      outputChannel,
    };

    client = new LanguageClient("handl-lsp", "handl LSP", serverOptions, clientOptions);
    console.log("[handl-lsp] client created, starting...");

    client.start().then(
      () => console.log("[handl-lsp] client started successfully"),
      (err: unknown) => {
        console.error("[handl-lsp] client failed to start:", err);
        const msg = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage("handl-lsp: " + msg);
      },
    );

    context.subscriptions.push(client);
  } catch (err) {
    console.error("[handl-lsp] activation error:", err);
    const msg = err instanceof Error ? err.message : String(err);
    vscode.window.showErrorMessage("handl-lsp: failed to start LSP client: " + msg);
  }
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}

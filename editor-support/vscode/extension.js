const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient");
const path = require("path");
const fs = require("fs");

// Dev workflow: run `npm run dev` in editor-support/vscode to package and
// install the extension, then reload the VS Code window
// (Command Palette > Developer: Reload Window) to pick up changes.

let client = null;

function resolveServerPath() {
  const config = vscode.workspace.getConfiguration("handl-lsp");
  const configured = config.get("server.path");
  if (configured) return configured;

  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) return null;

  const candidate = path.join(folders[0].uri.fsPath, "target", "debug", "handl-host-lsp");
  if (fs.existsSync(candidate)) return candidate;

  return null;
}

function activate(context) {
  const serverPath = resolveServerPath();
  if (!serverPath) {
    vscode.window.showWarningMessage(
      "handl-lsp: server binary not found. Build it with `cargo build -p handl-host-lsp` or set `handl-lsp.server.path`."
    );
    return;
  }

  console.log("[handl-lsp] server path:", serverPath);

  try {
    const { LanguageClient, TransportKind } = require("vscode-languageclient");
    console.log("[handl-lsp] vscode-languageclient loaded");

    const serverOptions = {
      command: serverPath,
      args: [],
      transport: TransportKind.stdio,
    };

    const outputChannel = vscode.window.createOutputChannel("handl LSP");
    console.log("[handl-lsp] output channel created");

    const clientOptions = {
      documentSelector: [{ scheme: "file", language: "handl" }],
      outputChannel: outputChannel,
    };

    client = new LanguageClient(
      "handl-lsp",
      "handl LSP",
      serverOptions,
      clientOptions
    );
    console.log("[handl-lsp] client created, starting...");

    client.start().then(
      () => console.log("[handl-lsp] client started successfully"),
      (err) => {
        console.error("[handl-lsp] client failed to start:", err);
        vscode.window.showErrorMessage("handl-lsp: " + err.message);
      }
    );

    context.subscriptions.push(client);
  } catch (err) {
    console.error("[handl-lsp] activation error:", err);
    vscode.window.showErrorMessage("handl-lsp: failed to start LSP client: " + err.message);
  }
}

function deactivate() {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

module.exports = { activate, deactivate };

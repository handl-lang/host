const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient");
const path = require("path");
const fs = require("fs");

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

  const serverOptions = {
    command: serverPath,
    args: [],
    transport: TransportKind.stdio,
  };

  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "handl" }],
  };

  const client = new LanguageClient(
    "handl-lsp",
    "handl LSP",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(client.start());
}

function deactivate() {}

module.exports = { activate, deactivate };

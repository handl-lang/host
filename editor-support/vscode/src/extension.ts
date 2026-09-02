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
      vscode.workspace.getConfiguration("handl") as unknown as {
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
  const config = vscode.workspace.getConfiguration("handl");
  const serverPath = resolveServerPath(depsFromContext());
  if (!serverPath) {
    vscode.window.showWarningMessage(
      "handl: server binary not found. Build it with `cargo build -p handl-host-lsp` or set `handl.server.path`.",
    );
    return;
  }

  console.log("[handl] server path:", serverPath);

  try {
    const serverOptions = {
      ...buildServerOptions(serverPath),
      transport: TransportKind.stdio,
    };

    const outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
    console.log("[handl] output channel created");

    const clientOptions = {
      documentSelector: [...DOCUMENT_SELECTOR],
      outputChannel,
      initializationOptions: {
        enableInlayHints: config.get("enableInlayHints", true),
        enableCodeLens: config.get("enableCodeLens", true),
        enableRefinementDiagnostics: config.get("enableRefinementDiagnostics", true),
        enableSemanticHighlighting: config.get("enableSemanticHighlighting", true),
      },
    };

    client = new LanguageClient("handl", "handl", serverOptions, clientOptions);
    console.log("[handl] client created, starting...");

    client.start().then(
      () => {
        console.log("[handl] client started successfully");

        // Explicitly register a formatting provider that delegates to the
        // LSP server's textDocument/formatting capability. The
        // vscode-languageclient auto-registration sometimes doesn't pick up
        // the documentFormattingProvider capability, so we register it
        // manually here.
        const formattingProvider: vscode.DocumentFormattingEditProvider = {
          provideDocumentFormattingEdits(
            document: vscode.TextDocument,
            _options: vscode.FormattingOptions,
            _token: vscode.CancellationToken,
          ): vscode.ProviderResult<vscode.TextEdit[]> {
            return client?.sendRequest<vscode.TextEdit[]>(
              "textDocument/formatting",
              {
                textDocument: { uri: document.uri.toString() },
                options: {
                  tabSize: 2,
                  insertSpaces: true,
                },
              },
              _token,
            );
          },
        };

        context.subscriptions.push(
          vscode.languages.registerDocumentFormattingEditProvider(
            DOCUMENT_SELECTOR,
            formattingProvider,
          ),
        );
        console.log("[handl] formatting provider registered");
      },
      (err: unknown) => {
        console.error("[handl] client failed to start:", err);
        const msg = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage("handl: " + msg);
      },
    );

    context.subscriptions.push(client);
  } catch (err) {
    console.error("[handl] activation error:", err);
    const msg = err instanceof Error ? err.message : String(err);
    vscode.window.showErrorMessage("handl: failed to start language server: " + msg);
  }
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}

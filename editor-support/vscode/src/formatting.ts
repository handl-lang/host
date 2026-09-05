import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { DOCUMENT_SELECTOR } from "./server";

export function registerFormatting(ctx: vscode.ExtensionContext, client: LanguageClient): void {
  const provider: vscode.DocumentFormattingEditProvider = {
    provideDocumentFormattingEdits(
      document: vscode.TextDocument,
      _options: vscode.FormattingOptions,
      token: vscode.CancellationToken,
    ): vscode.ProviderResult<vscode.TextEdit[]> {
      return client.sendRequest<vscode.TextEdit[]>(
        "textDocument/formatting",
        {
          textDocument: { uri: document.uri.toString() },
          options: { tabSize: 2, insertSpaces: true },
        },
        token,
      );
    },
  };

  ctx.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(DOCUMENT_SELECTOR, provider),
  );
}

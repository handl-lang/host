import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";
import { DOCUMENT_SELECTOR, OUTPUT_CHANNEL_NAME, buildServerOptions } from "./server";
import type { HandlConfig } from "./config";

export function createClient(serverPath: string, config: HandlConfig): LanguageClient {
  const serverOptions = {
    ...buildServerOptions(serverPath),
    transport: TransportKind.stdio,
  };

  const outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);

  const clientOptions = {
    documentSelector: [...DOCUMENT_SELECTOR],
    outputChannel,
    initializationOptions: {
      enableInlayHints: config.enableInlayHints,
      enableCodeLens: config.enableCodeLens,
      enableRefinementDiagnostics: config.enableRefinementDiagnostics,
      enableSemanticHighlighting: config.enableSemanticHighlighting,
    },
  };

  return new LanguageClient("handl", "handl", serverOptions, clientOptions);
}

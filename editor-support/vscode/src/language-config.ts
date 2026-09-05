import * as vscode from "vscode";

export function registerLanguageConfiguration(ctx: vscode.ExtensionContext): void {
  const config: vscode.LanguageConfiguration = {
    comments: {
      lineComment: "--",
      blockComment: ["---", "---"],
    },
    brackets: [
      ["{", "}"],
      ["[", "]"],
      ["(", ")"],
    ],
    autoClosingPairs: [
      { open: "{", close: "}" },
      { open: "[", close: "]" },
      { open: "(", close: ")" },
      { open: '"', close: '"', notIn: [vscode.SyntaxTokenType.String] },
      { open: "'", close: "'", notIn: [vscode.SyntaxTokenType.String] },
    ],
    indentationRules: {
      increaseIndentPattern: /^\s*(loop|do|match|rec|while|for|if|trait|impl|law|infer|type)\b.*$/,
      decreaseIndentPattern: /^\s*[}\])].*$/,
    },
    onEnterRules: [
      {
        beforeText: /--.*$/,
        action: { indentAction: vscode.IndentAction.None, appendText: "-- " },
      },
      {
        beforeText: /---.*$/,
        action: { indentAction: vscode.IndentAction.None, appendText: "--- " },
      },
    ],
  };

  ctx.subscriptions.push(vscode.languages.setLanguageConfiguration("handl", config));
}

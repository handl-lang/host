"use strict";

function makeClass() {
  return class {
    constructor(..._args) {}
  };
}

const real = {
  workspace: {
    getConfiguration: () => ({
      get: (_key, fallback) => fallback,
    }),
    workspaceFolders: undefined,
    fs: {
      stat: () => Promise.resolve(),
    },
  },
  window: {
    createOutputChannel: () => ({
      appendLine: () => {},
      show: () => {},
      dispose: () => {},
    }),
    showWarningMessage: () => undefined,
    showErrorMessage: () => undefined,
  },
  languages: {
    registerDocumentFormattingEditProvider: () => ({ dispose: () => {} }),
  },
  Uri: {
    file: (p) => ({ fsPath: p, toString: () => p }),
  },
};

const handler = {
  get(_target, prop) {
    if (prop in real) return real[prop];
    return makeClass();
  },
};

module.exports = new Proxy({}, handler);

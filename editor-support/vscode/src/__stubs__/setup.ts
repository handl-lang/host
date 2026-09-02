import Module from "module";
import { fileURLToPath } from "url";
import path from "path";

const stubPath = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "vscode.js");

const mod = Module as unknown as {
  _resolveFilename: (request: string, ...args: unknown[]) => string;
};
const originalResolveFilename = mod._resolveFilename;
mod._resolveFilename = function (request: string, ...args: unknown[]): string {
  if (request === "vscode") {
    return stubPath;
  }
  return originalResolveFilename.call(this, request, ...args);
};

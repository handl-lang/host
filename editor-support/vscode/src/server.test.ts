import { describe, it, expect } from "vitest";
import * as path from "path";
import {
  buildServerOptions,
  DOCUMENT_SELECTOR,
  OUTPUT_CHANNEL_NAME,
  resolveServerPath,
  type ResolveDeps,
} from "./server";

function makeDeps(overrides: Partial<ResolveDeps> = {}): ResolveDeps {
  return {
    getConfig: () => ({ "server.path": "" }),
    getWorkspaceFolders: () => undefined,
    exists: () => false,
    ...overrides,
  };
}

describe("resolveServerPath", () => {
  it("returns the configured path when set", () => {
    const deps = makeDeps({
      getConfig: () => ({ "server.path": "/custom/bin/handl-host-lsp" }),
    });
    expect(resolveServerPath(deps)).toBe("/custom/bin/handl-host-lsp");
  });

  it("returns null when no workspace folders", () => {
    expect(resolveServerPath(makeDeps())).toBeNull();
  });

  it("returns null when workspace folder exists but binary does not", () => {
    const deps = makeDeps({
      getWorkspaceFolders: () => [{ uri: { fsPath: "/my/project" } }],
      exists: () => false,
    });
    expect(resolveServerPath(deps)).toBeNull();
  });

  it("returns the workspace target/debug path when binary exists", () => {
    const deps = makeDeps({
      getWorkspaceFolders: () => [{ uri: { fsPath: "/my/project" } }],
      exists: (p) => p === path.join("/my/project", "target", "debug", "handl-host-lsp"),
    });
    const expected = path.join("/my/project", "target", "debug", "handl-host-lsp");
    expect(resolveServerPath(deps)).toBe(expected);
  });

  it("uses the first workspace folder when multiple exist", () => {
    const deps = makeDeps({
      getWorkspaceFolders: () => [{ uri: { fsPath: "/first" } }, { uri: { fsPath: "/second" } }],
      exists: (p) => p.startsWith("/first/"),
    });
    expect(resolveServerPath(deps)).toBe(path.join("/first", "target", "debug", "handl-host-lsp"));
  });

  it("configured path takes priority over workspace auto-detection", () => {
    const deps = makeDeps({
      getConfig: () => ({ "server.path": "/explicit/path" }),
      getWorkspaceFolders: () => [{ uri: { fsPath: "/workspace" } }],
      exists: () => true,
    });
    expect(resolveServerPath(deps)).toBe("/explicit/path");
  });
});

describe("buildServerOptions", () => {
  it("builds options with the given command and empty args", () => {
    const opts = buildServerOptions("/usr/bin/handl-host-lsp");
    expect(opts.command).toBe("/usr/bin/handl-host-lsp");
    expect(opts.args).toEqual([]);
  });
});

describe("constants", () => {
  it("DOCUMENT_SELECTOR targets file scheme and handl language", () => {
    expect(DOCUMENT_SELECTOR).toEqual([{ scheme: "file", language: "handl" }]);
  });

  it("OUTPUT_CHANNEL_NAME is handl", () => {
    expect(OUTPUT_CHANNEL_NAME).toBe("handl");
  });
});

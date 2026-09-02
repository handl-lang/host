import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { parseToml, readManifest, DEFAULT_EDITOR_CONFIG } from "./manifest";

describe("parseToml", () => {
  it("parseToml_empty_string_returns_empty_object", () => {
    expect(parseToml("")).toEqual({});
  });

  it("parseToml_comment_lines_are_ignored", () => {
    const content = [
      "# this is a comment",
      "",
      "# another comment",
      "[editor]",
      "# trailing comment",
      "enableInlayHints = true  # trailing comment after value",
    ].join("\n");
    const manifest = parseToml(content);
    expect(manifest.editor?.enableInlayHints).toBe(true);
  });

  it("parseToml_string_value", () => {
    const manifest = parseToml('[language]\nkit = "handl-kit-data-canonical"');
    expect(manifest.language?.kit).toBe("handl-kit-data-canonical");
  });

  it("parseToml_boolean_value", () => {
    const manifest = parseToml("[editor]\nenableInlayHints = true\nenableCodeLens = false");
    expect(manifest.editor?.enableInlayHints).toBe(true);
    expect(manifest.editor?.enableCodeLens).toBe(false);
  });

  it("parseToml_array_value", () => {
    const manifest = parseToml('[language]\nextensions = [".hnl", ".hn2", ".handl"]');
    expect(manifest.language?.extensions).toEqual([".hnl", ".hn2", ".handl"]);
  });

  it("parseToml_multiple_sections", () => {
    const content = [
      '[language]',
      'kit = "handl-kit-data-canonical"',
      'version = "0.1"',
      'extensions = [".hnl", ".hn2"]',
      '[editor]',
      'enableInlayHints = true',
      'enableCodeLens = false',
      'enableRefinementDiagnostics = true',
      'enableSemanticHighlighting = false',
    ].join("\n");
    const manifest = parseToml(content);
    expect(manifest.language?.kit).toBe("handl-kit-data-canonical");
    expect(manifest.language?.version).toBe("0.1");
    expect(manifest.language?.extensions).toEqual([".hnl", ".hn2"]);
    expect(manifest.editor?.enableInlayHints).toBe(true);
    expect(manifest.editor?.enableCodeLens).toBe(false);
    expect(manifest.editor?.enableRefinementDiagnostics).toBe(true);
    expect(manifest.editor?.enableSemanticHighlighting).toBe(false);
  });

  it("parseToml_unknown_sections_are_ignored", () => {
    const content = [
      "[server]",
      'path = "/custom/bin"',
      "[editor]",
      "enableInlayHints = true",
    ].join("\n");
    const manifest = parseToml(content);
    expect((manifest as { server?: unknown }).server).toBeUndefined();
    expect(manifest.editor?.enableInlayHints).toBe(true);
  });
});

describe("readManifest", () => {
  function withTempDir<T>(fn: (dir: string) => T): T {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "handl-manifest-"));
    try {
      return fn(dir);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }

  it("readManifest_no_file_returns_null", () => {
    withTempDir((dir) => {
      expect(readManifest(dir)).toBeNull();
    });
  });

  it("readManifest_valid_file_returns_manifest", () => {
    withTempDir((dir) => {
      const content = [
        '[language]',
        'kit = "handl-kit-data-canonical"',
        'version = "0.1"',
        'extensions = [".hnl", ".hn2"]',
        '[editor]',
        'enableInlayHints = true',
        'enableCodeLens = false',
      ].join("\n");
      fs.writeFileSync(path.join(dir, ".handl.toml"), content);
      const manifest = readManifest(dir);
      expect(manifest).not.toBeNull();
      expect(manifest?.language?.kit).toBe("handl-kit-data-canonical");
      expect(manifest?.language?.version).toBe("0.1");
      expect(manifest?.language?.extensions).toEqual([".hnl", ".hn2"]);
      expect(manifest?.editor?.enableInlayHints).toBe(true);
      expect(manifest?.editor?.enableCodeLens).toBe(false);
    });
  });

  it("readManifest_partial_manifest_only_editor", () => {
    withTempDir((dir) => {
      const content = ["[editor]", "enableInlayHints = false"].join("\n");
      fs.writeFileSync(path.join(dir, ".handl.toml"), content);
      const manifest = readManifest(dir);
      expect(manifest?.language).toBeUndefined();
      expect(manifest?.editor?.enableInlayHints).toBe(false);
      expect(manifest?.editor?.enableCodeLens).toBeUndefined();
    });
  });

  it("readManifest_partial_manifest_only_language", () => {
    withTempDir((dir) => {
      const content = ['[language]', 'kit = "my-kit"', 'extensions = [".hnl"]'].join("\n");
      fs.writeFileSync(path.join(dir, ".handl.toml"), content);
      const manifest = readManifest(dir);
      expect(manifest?.editor).toBeUndefined();
      expect(manifest?.language?.kit).toBe("my-kit");
      expect(manifest?.language?.extensions).toEqual([".hnl"]);
    });
  });

  it("readManifest_empty_workspace_root_returns_null", () => {
    expect(readManifest("")).toBeNull();
  });
});

describe("DEFAULT_EDITOR_CONFIG", () => {
  it("all flags default to true", () => {
    expect(DEFAULT_EDITOR_CONFIG.enableInlayHints).toBe(true);
    expect(DEFAULT_EDITOR_CONFIG.enableCodeLens).toBe(true);
    expect(DEFAULT_EDITOR_CONFIG.enableRefinementDiagnostics).toBe(true);
    expect(DEFAULT_EDITOR_CONFIG.enableSemanticHighlighting).toBe(true);
  });
});

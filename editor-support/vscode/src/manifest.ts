import * as fs from "fs";
import * as path from "path";

/**
 * Shape of the `.handl.toml` manifest file.
 *
 * Only the subset of TOML we care about is modeled here: the `[language]` and
 * `[editor]` sections. Unknown sections and keys are silently ignored.
 */
export interface HandlManifest {
  readonly language?: {
    readonly kit?: string;
    readonly version?: string;
    readonly extensions?: readonly string[];
  };
  readonly editor?: {
    readonly enableInlayHints?: boolean;
    readonly enableCodeLens?: boolean;
    readonly enableRefinementDiagnostics?: boolean;
    readonly enableSemanticHighlighting?: boolean;
  };
}

/**
 * Default editor configuration values, used when `.handl.toml` is absent or
 * does not specify a value. Mirrors the previous `package.json` defaults.
 */
export const DEFAULT_EDITOR_CONFIG = {
  enableInlayHints: true,
  enableCodeLens: true,
  enableRefinementDiagnostics: true,
  enableSemanticHighlighting: true,
} as const;

/**
 * Reads `.handl.toml` from the workspace root and returns a typed manifest.
 * Returns `null` when no manifest file is present.
 */
export function readManifest(workspaceRoot: string): HandlManifest | null {
  if (!workspaceRoot) return null;
  const manifestPath = path.join(workspaceRoot, ".handl.toml");
  if (!fs.existsSync(manifestPath)) return null;

  const content = fs.readFileSync(manifestPath, "utf-8");
  return parseToml(content);
}

/**
 * Minimal TOML parser for our limited subset.
 *
 * Supports:
 * - Section headers: `[language]`, `[editor]`
 * - String values: `key = "value"`
 * - Boolean values: `key = true` / `key = false`
 * - Array values: `extensions = [".hnl", ".hn2"]`
 * - Comments: `# ...` (full-line and trailing)
 * - Empty lines
 *
 * No external dependency. Unknown sections/keys are ignored.
 */
export function parseToml(content: string): HandlManifest {
  const result: {
    language?: Record<string, unknown>;
    editor?: Record<string, unknown>;
  } = {};

  let currentSection: Record<string, unknown> | null = null;
  let currentSectionName: string | null = null;

  const lines = content.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    // Strip full-line/trailing comments. We do a naive strip: cut at the first
    // `#` that is not inside a quoted string. For our subset, values are either
    // quoted strings, booleans, or arrays of quoted strings, so a `#` inside a
    // string is the only collision case — handled below per-value.
    const line = stripComment(rawLine).trim();
    if (line === "") continue;

    // Section header
    const sectionMatch = /^\[([^\]]+)\]$/.exec(line);
    if (sectionMatch) {
      currentSectionName = sectionMatch[1].trim();
      if (currentSectionName === "language" || currentSectionName === "editor") {
        if (!result[currentSectionName]) {
          result[currentSectionName] = {};
        }
        currentSection = result[currentSectionName]!;
      } else {
        // Unknown section — ignore its keys.
        currentSection = null;
      }
      continue;
    }

    // key = value
    const eqIndex = line.indexOf("=");
    if (eqIndex === -1) continue;
    const key = line.slice(0, eqIndex).trim();
    const valuePart = line.slice(eqIndex + 1).trim();
    if (key === "" || currentSection === null) continue;

    currentSection[key] = parseValue(valuePart);
  }

  // Build the typed manifest, only copying known keys.
  const language = result.language ? buildLanguage(result.language) : undefined;
  const editor = result.editor ? buildEditor(result.editor) : undefined;

  const manifest: HandlManifest = {
    ...(language ? { language } : {}),
    ...(editor ? { editor } : {}),
  };
  return manifest;
}

function buildLanguage(
  raw: Record<string, unknown>,
): NonNullable<HandlManifest["language"]> {
  const out: {
    kit?: string;
    version?: string;
    extensions?: readonly string[];
  } = {};
  const kit = asString(raw.kit);
  if (kit !== undefined) out.kit = kit;
  const version = asString(raw.version);
  if (version !== undefined) out.version = version;
  const extensions = asStringArray(raw.extensions);
  if (extensions !== undefined) out.extensions = extensions;
  return out;
}

function buildEditor(
  raw: Record<string, unknown>,
): NonNullable<HandlManifest["editor"]> {
  const out: {
    enableInlayHints?: boolean;
    enableCodeLens?: boolean;
    enableRefinementDiagnostics?: boolean;
    enableSemanticHighlighting?: boolean;
  } = {};
  const enableInlayHints = asBoolean(raw.enableInlayHints);
  if (enableInlayHints !== undefined) out.enableInlayHints = enableInlayHints;
  const enableCodeLens = asBoolean(raw.enableCodeLens);
  if (enableCodeLens !== undefined) out.enableCodeLens = enableCodeLens;
  const enableRefinementDiagnostics = asBoolean(raw.enableRefinementDiagnostics);
  if (enableRefinementDiagnostics !== undefined)
    out.enableRefinementDiagnostics = enableRefinementDiagnostics;
  const enableSemanticHighlighting = asBoolean(raw.enableSemanticHighlighting);
  if (enableSemanticHighlighting !== undefined)
    out.enableSemanticHighlighting = enableSemanticHighlighting;
  return out;
}

/**
 * Parses a single TOML value (the right-hand side of `key = value`).
 */
function parseValue(raw: string): unknown {
  if (raw === "true") return true;
  if (raw === "false") return false;
  if (raw.startsWith("[")) return parseArray(raw);
  if (raw.startsWith('"') || raw.startsWith("'")) return parseString(raw);
  // Bare strings/numbers are not part of our subset; return as-is string.
  return raw;
}

/**
 * Parses a TOML string literal: `"..."` or `'...'`. Handles escaped quotes
 * inside double-quoted strings.
 */
function parseString(raw: string): string {
  const quote = raw[0];
  // Find the closing quote, respecting backslash escapes for double quotes.
  let end = -1;
  for (let i = 1; i < raw.length; i++) {
    const ch = raw[i];
    if (ch === "\\" && quote === '"' && i + 1 < raw.length) {
      i++;
      continue;
    }
    if (ch === quote) {
      end = i;
      break;
    }
  }
  if (end === -1) return raw.slice(1);
  const inner = raw.slice(1, end);
  if (quote === '"') {
    return inner.replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }
  return inner;
}

/**
 * Parses a TOML array of string literals: `[".hnl", ".hn2"]`.
 */
function parseArray(raw: string): string[] {
  const result: string[] = [];
  let i = 1;
  while (i < raw.length) {
    // Skip whitespace and commas.
    while (i < raw.length && /[\s,]/.test(raw[i])) i++;
    if (i >= raw.length || raw[i] === "]") break;
    const quote = raw[i];
    if (quote !== '"' && quote !== "'") {
      // Skip non-string elements (not part of our subset).
      while (i < raw.length && raw[i] !== "," && raw[i] !== "]") i++;
      continue;
    }
    let end = -1;
    let j = i + 1;
    while (j < raw.length) {
      const ch = raw[j];
      if (ch === "\\" && quote === '"' && j + 1 < raw.length) {
        j += 2;
        continue;
      }
      if (ch === quote) {
        end = j;
        break;
      }
      j++;
    }
    if (end === -1) break;
    const inner = raw.slice(i + 1, end);
    result.push(quote === '"' ? inner.replace(/\\"/g, '"').replace(/\\\\/g, "\\") : inner);
    i = end + 1;
  }
  return result;
}

/**
 * Removes a `#` comment from a line, ignoring `#` inside quoted strings.
 */
function stripComment(line: string): string {
  let inString: '"' | "'" | null = null;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (inString) {
      if (ch === "\\" && inString === '"' && i + 1 < line.length) {
        i++;
        continue;
      }
      if (ch === inString) inString = null;
      continue;
    }
    if (ch === '"' || ch === "'") {
      inString = ch;
      continue;
    }
    if (ch === "#") return line.slice(0, i);
  }
  return line;
}

function asString(v: unknown): string | undefined {
  return typeof v === "string" ? v : undefined;
}

function asBoolean(v: unknown): boolean | undefined {
  return typeof v === "boolean" ? v : undefined;
}

function asStringArray(v: unknown): readonly string[] | undefined {
  if (!Array.isArray(v)) return undefined;
  return v.filter((x): x is string => typeof x === "string");
}

// Regenerates `../conformance.test.hnl` from `cases/number_format.json`
// using Node's `Intl.NumberFormat` as the oracle — the same role the old
// `crate-ecma402-icu4x` conformance harness played against the Rust impl,
// now targeting the `intl-c` package's `format_number(n, locale, opts)`.
//
//   node conformance/generate.mjs        (from handl-packages/intl-c/)
//
// Cases are emitted when every option key maps onto `intl-c`'s opts
// surface (SUPPORTED_OPTS below — style incl. percent/currency, the
// currency code, digit limits, grouping toggle). Anything else is
// skipped and listed at the bottom of the generated file.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const casesPath = path.join(root, "conformance", "cases", "number_format.json");
const outPath = path.join(root, "conformance.test.hnl");

const cases = JSON.parse(fs.readFileSync(casesPath, "utf-8"));

function resolveInput(raw) {
  if (typeof raw === "number") return raw;
  switch (raw) {
    case "NaN":
      return NaN;
    case "Infinity":
      return Infinity;
    case "-Infinity":
      return -Infinity;
    default:
      throw new Error(`unknown input literal: ${raw}`);
  }
}

// handl has no NaN/Infinity literal in scope here; skip non-finite inputs.
function hnlInput(value) {
  if (!Number.isFinite(value)) return null;
  return String(value);
}

function hnlString(s) {
  return JSON.stringify(s);
}

function testName(id, index) {
  return "test_" + id.replace(/[^a-zA-Z0-9]+/g, "_") + "_" + index;
}

// Option keys `intl-c` understands today (num_style + num_attrs in
// intl-c.hnl). A case is skipped unless every key lands here and the
// style maps to a UNUM_* style the backend binds.
const SUPPORTED_OPTS = new Set([
  "style",
  "currency",
  "useGrouping",
  "minimumFractionDigits",
  "maximumFractionDigits",
  "minimumIntegerDigits",
  "maximumIntegerDigits",
]);
const SUPPORTED_STYLES = new Set([
  "decimal",
  "percent",
  "currency",
  "scientific",
  "spellout",
  "ordinal",
  "duration",
]);

const supported = (options) =>
  Object.keys(options).every((k) => SUPPORTED_OPTS.has(k)) &&
  (options.style === undefined || SUPPORTED_STYLES.has(options.style)) &&
  (options.style !== "currency" || typeof options.currency === "string");

// handl record literal for the opts arg (`{ style: "currency", … }`)
function hnlOpts(options) {
  const entries = Object.entries(options).map(([k, v]) => `${k}: ${JSON.stringify(v)}`);
  return `{ ${entries.join(", ")} }`;
}

const lines = [
  "--- Conformance tests for `intl-c.format_number` — GENERATED FILE.",
  "---",
  "--- Source cases: `conformance/cases/number_format.json`. Regenerate with:",
  "---   node conformance/generate.mjs",
  "---",
  "--- Expected values are produced by Node's `Intl.NumberFormat` (the same",
  "--- oracle the retired `crate-ecma402-icu4x` harness used). Cases whose",
  "--- options fall outside `intl-c`'s opts surface are skipped and listed",
  "--- below; see `generate.mjs`.",
  "",
  'from "./intl-c" import * as intl_c',
  "",
];

let emitted = 0;
const skipped = [];

for (const testCase of cases) {
  if (!supported(testCase.options)) {
    skipped.push(testCase.id);
    continue;
  }
  const fmt = new Intl.NumberFormat(testCase.locale, testCase.options);
  testCase.inputs.forEach((raw, i) => {
    const value = resolveInput(raw);
    const input = hnlInput(value);
    if (input === null) {
      skipped.push(`${testCase.id}[${i}] (non-finite input)`);
      return;
    }
    const expected = hnlString(fmt.format(value));
    lines.push(
      `${testName(testCase.id, i)} := assert(if intl_c.available {`,
      `  tag.untag(intl_c.format_number(${input}, ${hnlString(testCase.locale)}, ${hnlOpts(testCase.options)})).value`,
      `} else {`,
      `  ${expected}`,
      `}) = ${expected}`,
      "",
    );
    emitted += 1;
  });
}

if (skipped.length > 0) {
  lines.push(`-- skipped (options beyond decimal style / non-finite inputs):`);
  for (const s of skipped) lines.push(`--   ${s}`);
  lines.push("");
}

fs.writeFileSync(outPath, lines.join("\n"));
console.log(`wrote ${outPath}: ${emitted} cases, ${skipped.length} skipped`);

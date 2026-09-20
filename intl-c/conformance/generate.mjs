// Regenerates `../conformance.test.hnl` from `cases/number_format.json`
// using Node's `Intl.NumberFormat` as the oracle — the same role the old
// `crate-ecma402-icu4x` conformance harness played against the Rust impl,
// now targeting the `intl-c` package's `format_number(n, locale)`.
//
//   node conformance/generate.mjs        (from handl-packages/intl-c/)
//
// Cases whose `options` go beyond `{ "style": "decimal" }` are skipped:
// the `intl` contract's `FormatNumber := (Value, Value)` takes only
// (number, locale) — there is no options parameter yet. Extend the
// contract + `intl-c` first, then unskip here.

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

const supported = (options) => Object.keys(options).length === 1 && options.style === "decimal";

const lines = [
  "--- Conformance tests for `intl-c.format_number` — GENERATED FILE.",
  "---",
  "--- Source cases: `conformance/cases/number_format.json`. Regenerate with:",
  "---   node conformance/generate.mjs",
  "---",
  "--- Expected values are produced by Node's `Intl.NumberFormat` (the same",
  "--- oracle the retired `crate-ecma402-icu4x` harness used). Cases needing",
  "--- NumberFormat options (percent, currency, ...) are skipped — the `intl`",
  "--- contract's `FormatNumber := (Value, Value)` takes only (number,",
  "--- locale); see `generate.mjs`.",
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
      `  tag.untag(intl_c.format_number(${input}, ${hnlString(testCase.locale)})).value`,
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

// Regenerates `../conformance.test.hnl` from `cases/*.json` using Node's
// `Intl` (ICU4C) as the oracle — the same role the retired
// `crate-ecma402-icu4x` conformance harness played against the Rust impl,
// now targeting the `intl-c` package's exports.
//
//   node conformance/generate.mjs        (from handl-packages/intl-c/)
//
// Each surface maps one ECMA-402 constructor/method onto an `intl_c.*`
// export. A case is emitted only when every option key lands in that
// surface's OPTS whitelist (what `intl-c` understands today); anything else
// is skipped and listed at the bottom of the generated file.
//
// Date cases always carry `timeZone: "UTC"` (injected below) — the oracle
// and the handl call must agree on zone regardless of the host's.
// Expected values assume host ICU ~ Node's bundled ICU (78.x); on a
// different ICU major some CLDR-derived strings legitimately drift.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const casesDir = path.join(root, "conformance", "cases");
const outPath = path.join(root, "conformance.test.hnl");

const loadCases = (name) =>
  JSON.parse(fs.readFileSync(path.join(casesDir, name + ".json"), "utf-8"));

// --- hnl literal rendering --------------------------------------------------

const IDENT = /^[a-zA-Z_][a-zA-Z0-9_]*$/;

function hnl(v) {
  if (v === null || v === undefined) return "null";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "number") {
    if (!Number.isFinite(v)) return null;
    return String(v);
  }
  if (typeof v === "string") return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(hnl).join(", ")}]`;
  return `{ ${Object.entries(v)
    .map(([k, x]) => `${IDENT.test(k) ? k : JSON.stringify(k)}: ${hnl(x)}`)
    .join(", ")} }`;
}

const hnlOpts = (options) => hnl(options && Object.keys(options).length ? options : null);

const optsOk = (options, allowed) => Object.keys(options ?? {}).every((k) => allowed.has(k));

const testName = (surface, id, i) =>
  "test_" + surface + "_" + id.replace(/[^a-zA-Z0-9]+/g, "_") + "_" + i;

// --- surfaces ---------------------------------------------------------------
//
// Each surface yields `{ name, call, expected }` per emitted test:
//   call     — hnl expression producing a `Result` (untagged via .value)
//   expected — JS value rendered as an hnl literal
// `inputs(c)` normalizes a case's variable field into a list of inputs.

const DATE_OPTS = new Set(["dateStyle", "timeStyle"]);
const NUM_OPTS = new Set([
  "style",
  "currency",
  "useGrouping",
  "minimumFractionDigits",
  "maximumFractionDigits",
  "minimumIntegerDigits",
  "maximumIntegerDigits",
]);
const NUM_STYLES = new Set([
  "decimal",
  "percent",
  "currency",
  "scientific",
  "spellout",
  "ordinal",
  "duration",
]);
const numOptsOk = (o) =>
  optsOk(o, new Set([...NUM_OPTS, "notation"])) &&
  (o.style === undefined || NUM_STYLES.has(o.style)) &&
  (o.notation === undefined || o.notation === "scientific") &&
  (o.style !== "currency" || typeof o.currency === "string");

// ECMA `notation: "scientific"` spells intl-c's `style: "scientific"`.
const intlcNumOpts = (o) => {
  if (!o) return null;
  const { notation, ...rest } = o;
  const mapped = { ...rest };
  if (notation === "scientific") mapped.style = "scientific";
  return Object.keys(mapped).length ? mapped : null;
};

const numCall = (fn, c, n) =>
  `intl_c.${fn}(${hnl(n)}, ${hnl(c.locale)}, ${hnl(intlcNumOpts(c.options))})`;

// V8 normalizes U+202F (NNBSP) and U+00A0 (NBSP) to ASCII space in Intl
// output; raw ICU4C emits them as-is. Tests wrap the result with the
// `conf_norm` helper emitted at the top of the file so both sides compare
// on the normalized form.
const normed = (expr) => `conf_norm(${expr})`;

const SURFACES = [
  {
    key: "number_format",
    file: "number_format",
    skip: (c) => !numOptsOk(c.options),
    inputs: (c) => c.inputs,
    tests: (c, n) => {
      if (typeof n !== "number" || !Number.isFinite(n)) return null;
      return {
        call: numCall("format_number", c, n),
        expected: new Intl.NumberFormat(c.locale, c.options).format(n),
      };
    },
  },
  {
    key: "number_parts",
    file: "number_format_parts",
    skip: (c) => !numOptsOk(c.options),
    inputs: (c) => c.inputs,
    tests: (c, n) => {
      if (typeof n !== "number" || !Number.isFinite(n)) return null;
      const parts = new Intl.NumberFormat(c.locale, c.options).formatToParts(n);
      return {
        call: numCall("format_number_to_parts", c, n),
        expected: parts.map(({ type, value }) => ({ type, value })),
      };
    },
  },
  {
    key: "collator",
    file: "collator",
    skip: (c) => !optsOk(c.options, new Set(["sensitivity", "caseFirst", "numeric"])),
    inputs: (c) => c.pairs,
    tests: (c, [a, b]) => ({
      call: `intl_c.compare_strings(${hnl(a)}, ${hnl(b)}, ${hnl(c.locale)}, ${hnlOpts(c.options)})`,
      expected: Math.sign(new Intl.Collator(c.locale, c.options).compare(a, b)),
    }),
  },
  {
    key: "plural",
    file: "plural",
    skip: (c) => !optsOk(c.options, new Set(["type"])),
    inputs: (c) => c.inputs,
    tests: (c, n) => {
      if (typeof n !== "number" || !Number.isFinite(n)) return null;
      return {
        call: numCall("select_plural", c, n),
        expected: new Intl.PluralRules(c.locale, c.options).select(n),
      };
    },
  },
  {
    key: "list",
    file: "list_format",
    skip: (c) => !optsOk(c.options, new Set(["type", "style"])),
    inputs: (c) => c.inputs,
    tests: (c, items) => ({
      call: `intl_c.format_list(${hnl(items)}, ${hnl(c.locale)}, ${hnlOpts(c.options)})`,
      expected: new Intl.ListFormat(c.locale, c.options).format(items),
    }),
  },
  {
    key: "reltime",
    file: "relative_time",
    skip: (c) => !optsOk(c.options, new Set(["numeric", "style"])),
    inputs: (c) => c.inputs,
    tests: (c, [n, unit]) => {
      if (typeof n !== "number" || !Number.isFinite(n)) return null;
      return {
        call: `intl_c.format_relative_time(${hnl(n)}, ${hnl(unit)}, ${hnl(c.locale)}, ${hnlOpts(c.options)})`,
        expected: new Intl.RelativeTimeFormat(c.locale, c.options).format(n, unit),
        norm: true,
      };
    },
  },
  {
    key: "date",
    file: "date_format",
    skip: (c) => !optsOk(c.options, DATE_OPTS),
    inputs: (c) => c.inputs,
    tests: (c, input) => {
      const ms = typeof input === "string" ? new Date(input).getTime() : input;
      if (!Number.isFinite(ms)) return null;
      const options = { ...c.options, timeZone: "UTC" };
      return {
        call: `intl_c.format_date(${hnl(input)}, ${hnl(c.locale)}, ${hnl(options)})`,
        expected: new Intl.DateTimeFormat(c.locale, options).format(ms),
        norm: true,
      };
    },
  },
  {
    key: "date_parts",
    file: "date_format_parts",
    skip: (c) => !optsOk(c.options, DATE_OPTS),
    inputs: (c) => c.inputs,
    tests: (c, input) => {
      const ms = typeof input === "string" ? new Date(input).getTime() : input;
      if (!Number.isFinite(ms)) return null;
      const options = { ...c.options, timeZone: "UTC" };
      const parts = new Intl.DateTimeFormat(c.locale, options).formatToParts(ms);
      return {
        call: `intl_c.format_date_to_parts(${hnl(input)}, ${hnl(c.locale)}, ${hnl(options)})`,
        expected: parts.map(({ type, value }) =>
          // NNBSP/NBSP live in `value` strings — normalize each part
          ({ type, value: value.replace(/[\u00a0\u202f]/g, " ") }),
        ),
        norm: true,
        norm_parts: true,
      };
    },
  },
  {
    key: "duration",
    file: "duration",
    skip: (c) => !optsOk(c.options, new Set(["style", "fractionalDigits"])),
    inputs: (c) => c.inputs,
    tests: (c, v) => ({
      call: `intl_c.format_duration(${hnl(v)}, ${hnl(c.locale)}, ${hnlOpts(c.options)})`,
      expected: new Intl.DurationFormat(c.locale, c.options).format(v),
      norm: true,
    }),
  },
  {
    key: "display",
    file: "display_names",
    skip: (c) => !optsOk(c.options, new Set(["type"])),
    inputs: (c) => c.inputs,
    tests: (c, code) => ({
      call: `intl_c.display_name(${hnl(code)}, ${hnl(c.options.type)}, ${hnl(c.locale)}, null)`,
      expected: new Intl.DisplayNames(c.locale, c.options).of(code),
    }),
  },
  {
    key: "segment",
    file: "segment",
    skip: () => false,
    inputs: (c) => c.inputs,
    tests: (c, text) => {
      const segs = [...new Intl.Segmenter(c.locale, { granularity: c.granularity }).segment(text)];
      return {
        call: `intl_c.segment(${hnl(text)}, ${hnl(c.granularity)}, ${hnl(c.locale)}, null)`,
        expected: segs.map(({ index, segment, isWordLike }) =>
          c.granularity === "word" ? { index, segment, isWordLike } : { index, segment },
        ),
      };
    },
  },
  {
    // One case file entry emits several field asserts per tag.
    key: "locale",
    file: "locale",
    skip: () => false,
    inputs: (c) => c.tags,
    tests: function* (c, tag) {
      const loc = new Intl.Locale(tag);
      const fields = [
        ["language", loc.language],
        ["script", loc.script ?? ""],
        ["region", loc.region ?? ""],
        ["canonical", Intl.getCanonicalLocales(tag)[0]],
      ];
      let i = 0;
      for (const [field, expected] of fields) {
        if (expected === undefined) continue;
        yield {
          name: `${testName("locale", c.id, i)}_${field}`,
          expr: `tag.untag(intl_c.locale(${hnl(tag)}, null)).value.${field}`,
          expected,
        };
        i += 1;
      }
    },
  },
  {
    key: "canonical",
    file: "canonical",
    skip: () => false,
    inputs: (c) => c.inputs,
    tests: (c, v) => {
      const expected = Intl.getCanonicalLocales(v);
      return {
        call: `intl_c.get_canonical_locales(${hnl(v)})`,
        expected: Array.isArray(v) ? expected : expected[0],
      };
    },
  },
  {
    key: "supported",
    file: "supported_locales",
    skip: () => false,
    inputs: (c) => c.inputs,
    tests: (c, req) => ({
      call: `intl_c.supported_locales_of(${hnl(req)}, null)`,
      expected: Intl.NumberFormat.supportedLocalesOf(req),
    }),
  },
];

// --- emission ---------------------------------------------------------------

const lines = [
  "--- Conformance tests for `intl-c` — GENERATED FILE.",
  "---",
  "--- Source cases: `conformance/cases/*.json`. Regenerate with:",
  "---   node conformance/generate.mjs",
  "---",
  "--- Expected values come from Node's `Intl` (ICU4C 78.x — the same role",
  "--- the retired `crate-ecma402-icu4x` harness's oracle played). Date cases",
  '--- pin timeZone: "UTC". Skipped cases (options outside `intl-c`\'s opts',
  "--- surface, non-finite inputs) are listed at the bottom.",
  "",
  'from "./intl-c" import * as intl_c',
  "",
  "-- V8 normalizes U+202F (NNBSP) and U+00A0 (NBSP) to ASCII space in Intl",
  "--- output; raw ICU4C emits them as-is. `conf_norm`/`conf_norm_parts` fold",
  "--- both sides onto the normalized form for string/part-list comparisons.",
  'conf_norm := (s) ~> str.replace(str.replace(s, char.from_code_point(8239), " "), char.from_code_point(160), " ")',
  "",
  "conf_norm_parts := (ps) ~> while { xs: ps, acc: [] } {",
  "  (xs, acc) ~> match xs {",
  "    []: { out: acc },",
  "    [h, ...t]: { state: { xs: t, acc: [...acc, { type: h.type, value: conf_norm(h.value) }] } }",
  "  }",
  "}",
  "",
];

let emitted = 0;
const skipped = [];

const emit = (name, expr, expected, t) => {
  const inner = t?.norm_parts ? `conf_norm_parts(${expr})` : t?.norm ? normed(expr) : expr;
  lines.push(
    `${name} := assert(if intl_c.available {`,
    `  ${inner}`,
    `} else {`,
    `  ${hnl(expected)}`,
    `}) = ${hnl(expected)}`,
    "",
  );
  emitted += 1;
};

for (const surface of SURFACES) {
  const cases = loadCases(surface.file);
  lines.push(`-- ${surface.key} ${"-".repeat(Math.max(4, 60 - surface.key.length))}`, "");
  for (const c of cases) {
    if (surface.skip(c)) {
      skipped.push(`${surface.key}:${c.id} (unsupported options)`);
      continue;
    }
    surface.inputs(c).forEach((input, i) => {
      const emitOne = (t, suffix) => {
        const name = t.name ?? testName(surface.key, c.id, i) + (suffix ?? "");
        const expr = t.expr ?? `tag.untag(${t.call}).value`;
        emit(name, expr, t.expected, t);
      };
      try {
        const out = surface.tests(c, input);
        if (out === null || out === undefined) {
          skipped.push(`${surface.key}:${c.id}[${i}] (unsupported input)`);
          return;
        }
        if (Symbol.iterator in Object(out)) {
          for (const t of out) emitOne(t);
        } else {
          emitOne(out);
        }
      } catch (e) {
        skipped.push(`${surface.key}:${c.id}[${i}] (oracle error: ${e.message})`);
      }
    });
  }
}

if (skipped.length > 0) {
  lines.push(`-- skipped (options beyond the intl-c opts surface / non-finite inputs):`);
  for (const s of skipped) lines.push(`--   ${s}`);
  lines.push("");
}

fs.writeFileSync(outPath, lines.join("\n"));
console.log(`wrote ${outPath}: ${emitted} cases, ${skipped.length} skipped`);

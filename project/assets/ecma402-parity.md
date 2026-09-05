# ECMA-402 Parity Table: `handl-lib-intl` vs Web `Intl` API

> **Last reviewed:** 2026-08-22 against `handl-lib-intl` source (ICU4X v2).
> See `packages/handl/crates/lib-intl/src/` for implementation.

## 1. `Intl.Collator` → `intl.compare_strings`

| ECMA-402 Option                                               | handl-lib-intl                                 | Status                            |
| ------------------------------------------------------------- | ---------------------------------------------- | --------------------------------- |
| `usage`: `"sort"` / `"search"`                                | ✅ `usage`                                     | ✅                                |
| `sensitivity`: `"base"` / `"accent"` / `"case"` / `"variant"` | ✅ `sensitivity`                               | ✅                                |
| `caseFirst`: `"upper"` / `"lower"` / `"false"`                | ✅ `caseFirst`                                 | ✅                                |
| `numeric`: boolean                                            | ✅ `numeric`                                   | ✅                                |
| `ignorePunctuation`: boolean                                  | ✅ `ignorePunctuation`                         | ✅                                |
| `collation`: string                                           | ✅ `collation` (phonebk, stroke, trad, pinyin) | ✅                                |
| `localeMatcher`                                               | ❌                                             | ⚠️ Low priority — always best-fit |

## 2. `Intl.DateTimeFormat` → `intl.format_date`

| ECMA-402 Option                                                        | handl-lib-intl                                            | Status          |
| ---------------------------------------------------------------------- | --------------------------------------------------------- | --------------- |
| `dateStyle`: `"full"` / `"long"` / `"medium"` / `"short"`              | ⚠️ `style` — `"full"` maps to `Long` (same as `"long"`)   | ⚠️ Partial      |
| `timeStyle`: `"full"` / `"long"` / `"medium"` / `"short"`              | ⚠️ `style` + `type: "time"` — `"full"` maps to `Long`     | ⚠️ Partial      |
| `hourCycle`: `"h11"` / `"h12"` / `"h23"` / `"h24"`                     | ✅ `hourCycle`                                            | ✅              |
| `hour12`: boolean                                                      | ✅ `hour12`                                               | ✅              |
| `calendar`: string                                                     | ✅ `calendar` (16 calendars incl. islamic variants)       | ✅              |
| `numberingSystem`: string                                              | ✅ `numberingSystem`                                      | ✅              |
| `timeZone`: IANA name                                                  | ⚠️ Parsed but not used — all formatting is timezone-naive | ⚠️ Partial      |
| `fractionalSecondDigits`: 1/2/3                                        | ✅ `fractionalSecondDigits`                               | ✅              |
| `weekday`: `"narrow"` / `"short"` / `"long"`                           | ✅ `weekday`                                              | ✅              |
| `era`: `"narrow"` / `"short"` / `"long"`                               | ✅ `era`                                                  | ✅              |
| `year`: `"numeric"` / `"2-digit"` / `"narrow"` / `"short"` / `"long"`  | ✅ `year`                                                 | ✅              |
| `month`: `"numeric"` / `"2-digit"` / `"narrow"` / `"short"` / `"long"` | ✅ `month`                                                | ✅              |
| `day`: `"numeric"` / `"2-digit"`                                       | ✅ `day`                                                  | ✅              |
| `dayPeriod`: `"narrow"` / `"short"` / `"long"`                         | ❌                                                        | ⚠️ Minor        |
| `hour`: `"numeric"` / `"2-digit"`                                      | ✅ `hour`                                                 | ✅              |
| `minute`: `"numeric"` / `"2-digit"`                                    | ✅ `minute`                                               | ✅              |
| `second`: `"numeric"` / `"2-digit"`                                    | ✅ `second`                                               | ✅              |
| `timeZoneName`: 6 variants                                             | ✅ `timeZoneName`                                         | ✅              |
| `formatMatcher`: `"basic"` / `"best fit"`                              | ❌                                                        | ⚠️ Low priority |
| `localeMatcher`                                                        | ❌                                                        | ⚠️ Low priority |
| `formatRange()`                                                        | ❌                                                        | ⚠️ Future       |
| `formatToParts()`                                                      | ❌                                                        | ⚠️ Future       |

## 3. `Intl.DisplayNames` → `intl.display_name`

| ECMA-402 Option                                              | handl-lib-intl                                                     | Status          |
| ------------------------------------------------------------ | ------------------------------------------------------------------ | --------------- |
| `type`: `"language"` / `"region"` / `"script"` / `"variant"` | ✅ All 4 implemented                                               | ✅              |
| `type`: `"currency"`                                         | ⚠️ Implemented but ignores blob provider — uses compiled data only | ⚠️ Bug          |
| `type`: `"calendar"`                                         | ⚠️ Implemented but hardcoded English strings, not locale-aware     | ⚠️ Partial      |
| `type`: `"dateTimeField"`                                    | ⚠️ Implemented but hardcoded English strings, not locale-aware     | ⚠️ Partial      |
| `style`: `"narrow"` / `"short"` / `"long"`                   | ✅ `style`                                                         | ✅              |
| `fallback`: `"code"` / `"none"`                              | ✅ `fallback`                                                      | ✅              |
| `languageDisplay`: `"dialect"` / `"standard"`                | ✅ `languageDisplay`                                               | ✅              |
| `localeMatcher`                                              | ❌                                                                 | ⚠️ Low priority |

## 4. `Intl.DurationFormat` → `intl.format_duration`

| ECMA-402 Option                                          | handl-lib-intl                                      | Status          |
| -------------------------------------------------------- | --------------------------------------------------- | --------------- |
| `style`: `"long"` / `"short"` / `"narrow"` / `"digital"` | ✅ `style`                                          | ✅              |
| `yearsStyle` … `nanosecondsStyle`                        | ✅ All 10 per-unit styles                           | ✅              |
| `yearsDisplay` … `nanosecondsDisplay`                    | ✅ All 10 per-unit displays                         | ✅              |
| `fractionalDigits`: number                               | ✅ `fractionalDigits`                               | ✅              |
| `localeMatcher`                                          | ❌                                                  | ⚠️ Low priority |
| **Blob provider**                                        | ❌ Panics — ICU4X `todo!()` in buffer provider path | ❌ **Bug**      |

## 5. `Intl.ListFormat` → `intl.format_list`

| ECMA-402 Option                                      | handl-lib-intl | Status          |
| ---------------------------------------------------- | -------------- | --------------- |
| `type`: `"conjunction"` / `"disjunction"` / `"unit"` | ✅ `type`      | ✅              |
| `style`: `"long"` / `"short"` / `"narrow"`           | ✅ `style`     | ✅              |
| `localeMatcher`                                      | ❌             | ⚠️ Low priority |
| `formatToParts()`                                    | ❌             | ⚠️ Future       |

> **Note:** Non-string items in the input array are silently filtered out via `filter_map`.

## 6. `Intl.NumberFormat` → `intl.format_number` / `intl.format_number_to_parts`

| ECMA-402 Option                                                           | handl-lib-intl                                                                                                                                                 | Status          |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- |
| `style`: `"decimal"` / `"percent"` / `"currency"` / `"unit"`              | ✅ All 4 styles                                                                                                                                                | ✅              |
| `currency`: ISO 4217                                                      | ✅ `currency`                                                                                                                                                  | ✅              |
| `currencyDisplay`: `"symbol"` / `"narrowSymbol"` / `"code"` / `"name"`    | ✅ `currencyDisplay`                                                                                                                                           | ✅              |
| `currencySign`: `"standard"` / `"accounting"`                             | ✅ `currencySign`                                                                                                                                              | ✅              |
| `unit`: sanctioned unit identifier                                        | ⚠️ ~15 units (meter, gram, kilogram, square-meter, liter, cubic-meter, duration units). Missing kilometer, degree-celsius, kilometer-per-hour, bit, byte, etc. | ⚠️ Partial      |
| `unitDisplay`: `"short"` / `"narrow"` / `"long"`                          | ✅ `unitDisplay`                                                                                                                                               | ✅              |
| `notation`: `"standard"` / `"scientific"` / `"engineering"` / `"compact"` | ⚠️ Scientific/engineering are hand-rolled (f64 log10/powi, hardcoded `"×10"` separator, not locale-aware)                                                      | ⚠️ Partial      |
| `compactDisplay`: `"short"` / `"long"`                                    | ✅ `compactDisplay`                                                                                                                                            | ✅              |
| `minimumIntegerDigits`                                                    | ✅ `minimumIntegerDigits`                                                                                                                                      | ✅              |
| `minimumFractionDigits`                                                   | ✅ `minimumFractionDigits`                                                                                                                                     | ✅              |
| `maximumFractionDigits`                                                   | ✅ `maximumFractionDigits`                                                                                                                                     | ✅              |
| `minimumSignificantDigits`                                                | ✅ `minimumSignificantDigits`                                                                                                                                  | ✅              |
| `maximumSignificantDigits`                                                | ✅ `maximumSignificantDigits`                                                                                                                                  | ✅              |
| `roundingIncrement`: 1-5000                                               | ❌                                                                                                                                                             | ⚠️ Minor        |
| `roundingMode`: 9 modes                                                   | ✅ `roundingMode`                                                                                                                                              | ✅              |
| `roundingPriority`: `"auto"` / `"morePrecision"` / `"lessPrecision"`      | ❌                                                                                                                                                             | ⚠️ Minor        |
| `trailingZeroDisplay`: `"auto"` / `"stripIfInteger"`                      | ❌                                                                                                                                                             | ⚠️ Minor        |
| `useGrouping`: boolean / `"min2"` / `"auto"` / `"always"`                 | ✅ All variants (boolean + string)                                                                                                                             | ✅              |
| `signDisplay`: 5 variants                                                 | ✅ `signDisplay`                                                                                                                                               | ✅              |
| `numberingSystem`: string                                                 | ✅ `numberingSystem`                                                                                                                                           | ✅              |
| `localeMatcher`                                                           | ❌                                                                                                                                                             | ⚠️ Low priority |
| `formatRange()`                                                           | ❌                                                                                                                                                             | ⚠️ Future       |
| `formatToParts()`                                                         | ✅ `intl.format_number_to_parts`                                                                                                                               | ✅              |

## 7. `Intl.PluralRules` → `intl.select_plural`

| ECMA-402 Option                    | handl-lib-intl                | Status          |
| ---------------------------------- | ----------------------------- | --------------- |
| `type`: `"cardinal"` / `"ordinal"` | ✅ `type`                     | ✅              |
| `minimumIntegerDigits`             | ✅ `minimumIntegerDigits`     | ✅              |
| `minimumFractionDigits`            | ✅ `minimumFractionDigits`    | ✅              |
| `maximumFractionDigits`            | ✅ `maximumFractionDigits`    | ✅              |
| `minimumSignificantDigits`         | ✅ `minimumSignificantDigits` | ✅              |
| `maximumSignificantDigits`         | ✅ `maximumSignificantDigits` | ✅              |
| `roundingIncrement`                | ❌                            | ⚠️ Minor        |
| `roundingMode`                     | ❌                            | ⚠️ Minor        |
| `roundingPriority`                 | ❌                            | ⚠️ Minor        |
| `trailingZeroDisplay`              | ❌                            | ⚠️ Minor        |
| `selectRange()`                    | ❌                            | ⚠️ Future       |
| `localeMatcher`                    | ❌                            | ⚠️ Low priority |

## 8. `Intl.RelativeTimeFormat` → `intl.format_relative_time`

| ECMA-402 Option                            | handl-lib-intl | Status          |
| ------------------------------------------ | -------------- | --------------- |
| `style`: `"long"` / `"short"` / `"narrow"` | ✅ `style`     | ✅              |
| `numeric`: `"always"` / `"auto"`           | ✅ `numeric`   | ✅              |
| `localeMatcher`                            | ❌             | ⚠️ Low priority |
| `formatToParts()`                          | ❌             | ⚠️ Future       |

## 9. `Intl.Segmenter` → `intl.segment`

| ECMA-402 Option                                                  | handl-lib-intl                    | Status          |
| ---------------------------------------------------------------- | --------------------------------- | --------------- |
| `granularity`: `"grapheme"` / `"word"` / `"sentence"` / `"line"` | ✅ `granularity` (positional arg) | ✅              |
| `lineBreakStyle`: `"strict"` / `"normal"` / `"loose"`            | ✅ `lineBreakStyle`               | ✅              |
| `localeMatcher`                                                  | ❌                                | ⚠️ Low priority |

> **Note:** Locale is parsed but not passed to `LineSegmenter`. Line break behavior can be locale-sensitive (e.g. Chinese vs English).

## 10. `Intl.Locale` → `intl.locale`

| ECMA-402                                                                           | handl-lib-intl                      | Status |
| ---------------------------------------------------------------------------------- | ----------------------------------- | ------ |
| Constructor (parse BCP 47 tag)                                                     | ✅ `intl.locale(tag, opts?)`        | ✅     |
| `baseName` / `language` / `script` / `region`                                      | ✅ All returned in result object    | ✅     |
| `calendar` / `numberingSystem` / `hourCycle`                                       | ✅ Set via opts, returned in result | ✅     |
| `collation` / `numeric` / `caseFirst`                                              | ❌ Not settable or returned         | ❌     |
| `maximize()` / `minimize()`                                                        | ❌                                  | ❌     |
| `getCalendars()` / `getCollations()` / `getHourCycles()` / `getNumberingSystems()` | ❌                                  | ❌     |
| `getTextInfo()` / `getTimeZones()` / `getWeekInfo()`                               | ❌                                  | ❌     |

## 11. `Intl.supportedValuesOf()` → `intl.supported_locales_of`

| ECMA-402                               | handl-lib-intl                                                                     | Status     |
| -------------------------------------- | ---------------------------------------------------------------------------------- | ---------- |
| `supportedLocalesOf()`                 | ⚠️ `intl.supported_locales_of` — hardcoded 72-language list, ignores blob provider | ⚠️ Partial |
| `supportedValuesOf("calendar")`        | ❌                                                                                 | ❌         |
| `supportedValuesOf("collation")`       | ❌                                                                                 | ❌         |
| `supportedValuesOf("currency")`        | ❌                                                                                 | ❌         |
| `supportedValuesOf("numberingSystem")` | ❌                                                                                 | ❌         |
| `supportedValuesOf("timeZone")`        | ❌                                                                                 | ❌         |
| `supportedValuesOf("unit")`            | ❌                                                                                 | ❌         |

## 12. `Intl.getCanonicalLocales` → `intl.get_canonical_locales`

| ECMA-402                    | handl-lib-intl                                                           | Status     |
| --------------------------- | ------------------------------------------------------------------------ | ---------- |
| Accepts string or array     | ✅                                                                       | ✅         |
| Returns canonicalized array | ⚠️ Silently returns original string on parse failure instead of throwing | ⚠️ Partial |

## 13. Cross-cutting gaps

| Feature                                        | Status                                                                    |
| ---------------------------------------------- | ------------------------------------------------------------------------- |
| `localeMatcher` (all constructors)             | ❌ Not implemented — always best-fit                                      |
| `formatToParts()`                              | ✅ NumberFormat only — missing for DateTime, List, RelativeTime, Duration |
| `formatRange()` (NumberFormat, DateTimeFormat) | ❌ Not implemented                                                        |
| `resolvedOptions()` (all constructors)         | ❌ Not implemented                                                        |
| `Intl.supportedValuesOf()`                     | ❌ Not implemented (only `supportedLocalesOf`)                            |

## 14. Performance concerns

| Issue                                                           | Location         | Impact                       |
| --------------------------------------------------------------- | ---------------- | ---------------------------- |
| `IntlProvider` clones entire blob Vec on every function call    | `provider.rs:36` | High for blob-provider users |
| No formatter caching — new ICU4X formatter constructed per call | All functions    | Medium for hot paths         |

---

## Summary

**✅ Fully implemented:** Collator, ListFormat, PluralRules, RelativeTimeFormat, Segmenter (minor: locale unused for line)

**⚠️ Partially implemented:**

- DateTimeFormat — `full` style ≡ `long`, timezone parsed but ignored, no `formatToParts`
- DisplayNames — currency ignores blob provider, calendar/dateTimeField hardcoded English
- DurationFormat — panics with blob provider
- NumberFormat — unit coverage limited (~15 units), scientific/engineering hand-rolled
- `Intl.Locale` — no maximize/minimize, no collation/numeric/caseFirst
- `supported_locales_of` — hardcoded list, ignores blob provider
- `get_canonical_locales` — silently passes invalid input

**❌ Not implemented:** `formatToParts` (non-number), `formatRange`, `resolvedOptions`, `Intl.supportedValuesOf`, `Intl.Locale.maximize/minimize`

**Bugs (ranked by severity):**

1. **`format_duration` panics with blob provider** — ICU4X `todo!()` in buffer provider path
2. **`display_name` currency ignores blob provider** — uses compiled data regardless of configuration
3. **`display_name` calendar/dateTimeField not locale-aware** — hardcoded English strings
4. **`supported_locales_of` ignores blob provider** — hardcoded 72-language list

**Most impactful remaining gaps (ranked):**

1. **`formatToParts` for DateTimeFormat** — needed for rich UI date formatting
2. **Timezone support in `format_date`** — offsets parsed but ignored, no IANA timezone support
3. **Unit coverage in NumberFormat** — only ~15 units, missing common ones (kilometer, celsius, km/h)
4. **Scientific/engineering notation** — hand-rolled, not locale-aware, float precision risks
5. **`full` date/time style** — currently identical to `long`
6. **`resolvedOptions()`** — needed for introspection
7. **Blob provider performance** — clones entire blob on every call

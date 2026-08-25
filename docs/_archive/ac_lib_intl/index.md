# handl_lib_intl

# handl-lib-intl

Internationalization library for the handl runtime, backed by ICU4X.

Provides locale-aware functions for formatting numbers, selecting
plural categories, formatting lists, relative time, dates, durations,
comparing strings, display names, and text segmentation.

## Architecture

```text
handl-runtime-core          handl-lib-intl
─────────────            ────────────
Module trait             IntlModule
ModuleHost               ICU4X compiled_data (600+ locales)
                         IntlError → VmError
                         *Function structs
```

All locale data is provided by ICU4X's `compiled_data` feature,
which bundles CLDR data at compile time. This gives full CLDR
coverage (600+ locales) with no runtime data loading.

For smaller binaries, use [`IntlModule::with_blob_provider`] to
load a subset of CLDR data at runtime from a `.postcard` blob.

## Functions

- [`IntlModule`] — `intl.format_number`, `intl.select_plural`,
  `intl.format_list`, `intl.format_relative_time`, `intl.format_date`,
  `intl.format_duration`, `intl.compare_strings`, `intl.display_name`,
  `intl.segment`

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_intl::IntlModule;

let host = ModuleHost::new()
    .with(IntlModule);
```

Expressions can then call `intl.format_number(1234567.89, "pt-BR")`,
`intl.select_plural(0, "ar")`, `intl.format_list(["a", "b", "c"], "en")`,
`intl.format_relative_time(5, "second")`, `intl.format_date("2024-05-08")`,
etc.

## Modules

### [`handl_lib_intl`](handl_lib_intl.md)

*16 modules*

### [`common`](common.md)

*1 enum, 19 functions*

### [`compare_strings`](compare_strings.md)

*1 struct*

### [`display_name`](display_name.md)

*1 struct, 2 functions*

### [`error`](error.md)

*1 enum*

### [`format_date`](format_date.md)

*2 structs, 27 functions*

### [`format_duration`](format_duration.md)

*1 constant, 1 struct, 3 functions*

### [`format_list`](format_list.md)

*1 struct*

### [`format_number`](format_number.md)

*1 constant, 2 structs, 20 functions*

### [`format_relative_time`](format_relative_time.md)

*1 function, 1 struct*

### [`locale`](locale.md)

*1 struct*

### [`locale_match`](locale_match.md)

*1 constant, 2 functions, 2 structs*

### [`module`](module.md)

*1 struct*

### [`module::IntlModule`](module/IntlModule.md)

*2 functions*

### [`parts`](parts.md)

*1 struct*

### [`provider`](provider.md)

*1 struct*

### [`segment`](segment.md)

*1 struct, 2 functions*

### [`select_plural`](select_plural.md)

*1 struct*


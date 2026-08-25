# handl_lib_utils

# handl-lib-utils

Standard library modules for the handl runtime.

Provides concrete [`Module`] implementations with useful functions for
expressions and interpolation:

- [`MathModule`] — `math.abs`, `math.floor`, `math.ceil`, `math.round`,
  `math.min`, `math.max`, `math.sqrt`, `math.pow`, `math.sign`, `math.trunc`,
  `math.clamp`, `math.log`, `math.log10`, `math.log2`, `math.exp`, `math.sin`,
  `math.cos`, `math.tan`, `math.asin`, `math.acos`, `math.atan`, `math.atan2`,
  `math.hypot`, `math.mod`, `math.cbrt`, `math.degrees`, `math.radians`,
  `math.round_to`, `math.pi`, `math.e`
- [`StrModule`] — `str.upper`, `str.lower`, `str.trim`,
  `str.trim_start`, `str.trim_end`, `str.split`, `str.join`,
  `str.replace`, `str.replace_first`, `str.contains`,
  `str.starts_with`, `str.ends_with`, `str.slice`, `str.reverse`,
  `str.repeat`, `str.find`, `str.count`, `str.char_at`,
  `str.pad_left`, `str.pad_right`, `str.length`, `str.concat`,
  `str.strip_prefix`, `str.strip_suffix`, `str.lines`, `str.rfind`,
  `str.capitalize`, `str.title_case`, `str.snake_case`, `str.camel_case`,
  `str.pascal_case`, `str.kebab_case`, `str.split_at`, `str.trim_matches`,
  `str.is_empty`, `str.equals`, `str.bytes`, `str.is_alpha`,
  `str.is_numeric`, `str.is_alphanumeric`, `str.levenshtein`,
  `str.similarity`, `str.split_whitespace`, `str.truncate`,
  `str.slugify`, `str.escape_html`, `str.unescape_html`,
  `str.escape_url`, `str.unescape_url`, `str.word_wrap`
- [`ArrayModule`] — `array.length`, `array.reverse`, `array.concat`,
  `array.contains`, `array.index_of`, `array.slice`, `array.first`,
  `array.last`, `array.sort`, `array.flatten`, `array.deep_flatten`,
  `array.sum`, `array.avg`, `array.min`, `array.max`,
  `array.at`, `array.push`, `array.pop`, `array.shift`, `array.fill`,
  `array.take`, `array.drop`, `array.take_while`, `array.drop_while`,
  `array.enumerate`, `array.unzip`, `array.scan`, `array.foldr`,
  `array.intersperse`, `array.intercalate`,
  `array.find_index`, `array.find_last`,
  `array.find_last_index`, `array.dedupe`, `array.prepend`,
  `array.windows`, `array.transpose`, `array.insert_at`, `array.remove_at`,
  `array.range`, `array.zip`, `array.chunk`, `array.map`, `array.filter`,
  `array.reduce`, `array.flat_map`, `array.find`, `array.some`, `array.every`,
  `array.group_by`, `array.partition`, `array.sort_by`, `array.sort_by_desc`,
  `array.compact`, `array.pluck`, `array.min_by`, `array.max_by`,
  `array.count_by`
- [`SetModule`] — `set.unique`, `set.union`, `set.intersection`,
  `set.difference`, `set.symmetric_difference`, `set.without`,
  `set.unique_by`
- [`BoolModule`] — `bool.and`, `bool.or`, `bool.not`, `bool.xor`,
  `bool.nand`, `bool.nor`, `bool.implies`, `bool.eq`
- [`CmpModule`] — `cmp.eq`, `cmp.ne`, `cmp.lt`, `cmp.le`, `cmp.gt`,
  `cmp.ge`, `cmp.compare`, `cmp.min`, `cmp.max`, `cmp.clamp`,
  `cmp.between`, `cmp.in_range`, `cmp.is_sorted`, `cmp.is_sorted_desc`,
  `cmp.sort`, `cmp.sort_desc`
- [`ObjModule`] — `obj.keys`, `obj.values`, `obj.entries`,
  `obj.from_entries`, `obj.pick`, `obj.omit`, `obj.merge`,
  `obj.deep_get`, `obj.deep_merge`, `obj.get`, `obj.get_or`,
  `obj.set`, `obj.update`, `obj.has_key`, `obj.map_values`,
  `obj.map_keys`, `obj.filter_keys`, `obj.filter_values`,
  `obj.invert`, `obj.path_or`, `obj.set_path`, `obj.update_path`,
  `obj.size`, `obj.clone_deep`
- [`FnModule`] — `fn.pipe`, `fn.flow`, `fn.compose`, `fn.fanout`,
  `fn.choice`, `fn.apply`, `fn.tap`, `fn.constantly`, `fn.flip`,
  `fn.partial`, `fn.when`, `fn.unless`, `fn.converge`
- [`TypeModule`] — `type.to_string`, `type.to_number`, `type.to_bool`,
  `type.is_null`, `type.is_bool`, `type.is_number`, `type.is_string`,
  `type.is_array`, `type.is_object`, `type.type_of`, `type.is_empty`,
  `type.is_truthy`, `type.identity`, `type.default`, `type.coalesce`
- [`OptionModule`] — `option.some`, `option.none`, `option.is_some`,
  `option.is_none`, `option.unwrap`, `option.unwrap_or`, `option.map`,
  `option.flat_map`, `option.filter`, `option.or`, `option.and`, `option.xor`
- [`ResultModule`] — `result.ok`, `result.err`, `result.is_ok`,
  `result.is_err`

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_utils::{MathModule, StrModule, ArrayModule, BoolModule, CmpModule,
    ObjModule, FnModule, TypeModule, OptionModule, SetModule};

let host = ModuleHost::new()
    .with(MathModule)
    .with(StrModule)
    .with(ArrayModule)
    .with(BoolModule)
    .with(CmpModule)
    .with(ObjModule)
    .with(FnModule)
    .with(TypeModule)
    .with(OptionModule)
    .with(SetModule);
```

This crate is interface-agnostic — usable from CLI, MCP, or any consumer
that evaluates handl expressions.

## Modules

### [`handl_lib_utils`](handl_lib_utils.md)

*13 modules*

### [`array`](array.md)

*6 modules*

### [`array::basic_operations`](array/basic_operations.md)

*16 structs*

### [`array::helpers`](array/helpers.md)

*1 macro*

### [`array::higher_order`](array/higher_order.md)

*19 structs, 2 functions*

### [`array::iteration`](array/iteration.md)

*19 structs*

### [`array::module`](array/module.md)

*1 struct*

### [`array::numeric_aggregates`](array/numeric_aggregates.md)

*8 structs*

### [`bool`](bool.md)

*1 module*

### [`bool::bool`](bool/bool.md)

*11 structs, 2 macros, 4 functions*

### [`cmp`](cmp.md)

*1 module*

### [`cmp::cmp`](cmp/cmp.md)

*17 structs, 8 functions*

### [`fn`](fn.md)

*2 modules*

### [`fn::combinators`](fn/combinators.md)

*15 structs, 5 functions*

### [`fn::module`](fn/module.md)

*1 struct*

### [`math`](math.md)

*8 modules*

### [`math::arithmetic`](math/arithmetic.md)

*1 function, 14 structs*

### [`math::constants`](math/constants.md)

*5 structs*

### [`math::exponential`](math/exponential.md)

*7 structs*

### [`math::helpers`](math/helpers.md)

*2 functions, 2 macros*

### [`math::module`](math/module.md)

*1 struct*

### [`math::predicates`](math/predicates.md)

*4 structs*

### [`math::rounding`](math/rounding.md)

*8 structs*

### [`math::trigonometry`](math/trigonometry.md)

*16 structs*

### [`mood`](mood.md)

*1 module*

### [`mood::mood`](mood/mood.md)

*10 functions, 7 structs*

### [`obj`](obj.md)

*8 modules*

### [`obj::access`](obj/access.md)

*5 structs*

### [`obj::combination`](obj/combination.md)

*3 structs*

### [`obj::common`](obj/common.md)

*15 functions*

### [`obj::inspection`](obj/inspection.md)

*4 structs*

### [`obj::module`](obj/module.md)

*1 struct*

### [`obj::mutation`](obj/mutation.md)

*4 structs*

### [`obj::selection`](obj/selection.md)

*2 structs*

### [`obj::transformation`](obj/transformation.md)

*6 structs*

### [`option`](option.md)

*2 modules*

### [`option::functions`](option/functions.md)

*15 structs, 5 functions*

### [`option::module`](option/module.md)

*1 struct*

### [`result`](result.md)

*2 modules*

### [`result::functions`](result/functions.md)

*1 function, 4 structs*

### [`result::module`](result/module.md)

*1 struct*

### [`set`](set.md)

*1 module*

### [`set::set`](set/set.md)

*8 functions, 8 structs*

### [`shared`](shared.md)

*28 functions*

### [`str`](str.md)

*1 struct, 5 modules*

### [`str::basic`](str/basic.md)

*15 structs*

### [`str::case`](str/case.md)

*1 function, 6 structs*

### [`str::common`](str/common.md)

*2 macros, 6 functions*

### [`str::search`](str/search.md)

*1 function, 8 structs*

### [`str::transform`](str/transform.md)

*2 functions, 21 structs*

### [`type`](type.md)

*4 modules*

### [`type::coercion`](type/coercion.md)

*2 functions, 3 structs*

### [`type::module`](type/module.md)

*1 struct*

### [`type::predicates`](type/predicates.md)

*1 macro, 3 functions, 9 structs*

### [`type::utilities`](type/utilities.md)

*2 functions, 3 structs*


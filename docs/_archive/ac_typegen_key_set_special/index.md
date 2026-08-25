# handl_typegen_key_set_special

# handl-typegen-key-set-special

Typegen key handling for the special handl key set.

Implements the [`KeyHandler`] trait from `handl-typegen-core`. Each
canonical handl key (`arg`, `expr`, `inheritable`, `merge`, `mutable`)
has its own module within this crate. The modules are thin — they define
a `KEY` constant and a `strip_key` function that delegates to
[`TypegenContext::strip_keys`]. Keys with additional behavior (`expr`,
`merge`, `arg`) also expose helper functions.

Depends on `handl-typegen-core` for generic primitives.

## Modules

### [`handl_typegen_key_set_special`](handl_typegen_key_set_special.md)

*1 struct, 5 modules*

### [`arg`](arg.md)

*2 functions*

### [`expr`](expr.md)

*2 functions*

### [`inheritable`](inheritable.md)

*1 function*

### [`merge`](merge.md)

*3 functions*

### [`mutable`](mutable.md)

*1 function*


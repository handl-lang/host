**handl_compiler > config**

# Module: config

## Contents

**Structs**

- [`Config`](#config) - Configuration for a compilation pipeline.

---

## handl_compiler::config::Config

*Struct*

Configuration for a compilation pipeline.

**Fields:**
- `dialect: Box<dyn Dialect>`
- `resolver: Box<dyn ModuleResolver>`
- `interpreter: Box<dyn SchemaInterpreter>`
- `backend: Option<Box<dyn TypegenBackend>>`
- `key_handler: Box<dyn KeyHandler>`
- `import_resolver: Box<dyn ImportResolver>`
- `extensions: std::collections::HashMap<std::any::TypeId, Box<dyn Any>>` - Extensible typed config — custom passes can read their config

**Methods:**

- `fn extension<T>(self: &Self) -> Option<&T>` - Borrow a typed extension previously added via [`Config::with_extension`].
- `fn with_extension<T>(self: Self, ext: T) -> Self` - Add a typed extension. Enables custom passes to receive config




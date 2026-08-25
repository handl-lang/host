**handl_lib_os > sig > sig_stub**

# Module: sig::sig_stub

## Contents

**Structs**

- [`StubSig`](#stubsig) - A signal service with a fixed set of supported signals.

---

## handl_lib_os::sig::sig_stub::StubSig

*Struct*

A signal service with a fixed set of supported signals.

**Fields:**
- `supported: Vec<String>`
- `log: std::sync::Mutex<Vec<(i64, String)>>`

**Methods:**

- `fn new(signals: Vec<String>) -> Self`

**Trait Implementations:**

- **Sig**
  - `fn send(self: &Self, process_id: i64, signal: &str) -> Result<(), OsError>`
  - `fn list(self: &Self) -> Result<Vec<String>, OsError>`
- **Default**
  - `fn default() -> Self`




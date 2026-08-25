**handl_lib_os_std > sig**

# Module: sig

## Contents

**Structs**

- [`StdSig`](#stdsig) - Signal implementation backed by platform signal APIs.

**Functions**

- [`parse_signal`](#parse_signal)
- [`validate_pid`](#validate_pid)

---

## handl_lib_os_std::sig::StdSig

*Struct*

Signal implementation backed by platform signal APIs.

**Unit Struct**

**Trait Implementations:**

- **Sig**
  - `fn send(self: &Self, process_id: i64, signal: &str) -> Result<(), OsError>`
  - `fn list(self: &Self) -> Result<Vec<String>, OsError>`
- **Default**
  - `fn default() -> StdSig`



## handl_lib_os_std::sig::parse_signal

*Function*

```rust
fn parse_signal(signal: &str) -> Result<nix::sys::signal::Signal, handl_lib_os::OsError>
```



## handl_lib_os_std::sig::validate_pid

*Function*

```rust
fn validate_pid(process_id: i64) -> Result<nix::unistd::Pid, handl_lib_os::OsError>
```




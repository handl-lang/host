# Miri compatibility notes for `handl-lib-concurrent`

## Running Miri

```text
cargo +nightly miri test -p handl-lib-concurrent
```

## Current status

Miri is not currently installed for the active nightly toolchain. Attempting to run the command above produces:

```text
error: 'cargo-miri' is not installed for the toolchain 'nightly-x86_64-unknown-linux-gnu'.
help: run `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu miri` to install it
```

Install Miri with:

```text
rustup component add --toolchain nightly-x86_64-unknown-linux-gnu miri
```

## Known Miri blockers

- **`parking_lot`** uses OS-level synchronization primitives that Miri cannot fully interpret.
- **`std::thread`** and associated `std::sync` APIs are not fully supported under Miri; some tests may hang or produce unsupported-operation errors.
- **`crossbeam-deque`** (used by the thread-pool scheduler) also relies on atomic/epoch mechanisms that can be slow or unsupported in Miri.

Once Miri is installed, run the targeted test command and record any new errors here.

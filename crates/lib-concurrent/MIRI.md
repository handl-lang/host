# Miri and Loom compatibility notes for `handl-lib-concurrent`

## Running Loom

Loom is a `dev-dependency` used only under the `loom` cfg flag. The bundled
runtimes use `std::sync` primitives that Loom cannot intercept, so the Loom
tests in `tests/loom.rs` exercise the same concurrency contracts with
Loom-native `loom::sync` types.

```text
LOOM_MAX_PREEMPTIONS=2 RUSTFLAGS="--cfg loom" cargo test -p handl-lib-concurrent --test loom
```

## Loom status

The `tests/loom.rs` suite currently passes with 4 model-checking tests:

- `atomic_counter_model` — two threads `fetch_add` into an `AtomicU64`.
- `channel_two_senders_one_receiver_model` — two senders into a `Mutex<VecDeque>`.
- `mutex_counter_model` — two threads increment a `Mutex<u64>`.
- `cancel_token_prevents_eval_when_cancelled` — a pre-cancelled token prevents the thunk from running.

Because the crate's `DefaultSyncRuntime`, `DefaultAtomicRuntime` and
`DefaultCancelRuntime` rely on `std::sync`/`parking_lot` types, the bundled
runtimes themselves are not exercised under Loom. The `tests/loom.rs` tests are
self-contained and use `loom` primitives directly.

## Running Miri

For the unit tests only (not property/stress tests):

```text
cargo +nightly miri test -p handl-lib-concurrent --lib
```

## Miri status

Miri is not currently installed for the active nightly toolchain. Attempting to
run the command above produces:

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

Once Miri is installed, run the targeted `cargo +nightly miri test -p handl-lib-concurrent --lib` command and record any new errors here.

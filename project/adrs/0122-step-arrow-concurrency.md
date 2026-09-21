# Step-Arrow Concurrency: Handl-Authored Scheduler over epoll

## Context

A native handl HTTP server needs concurrency, but neither exec engine can
suspend a running computation (`plugin-engine-exec-bytecode` and
`plugin-engine-exec-tree-walk` have no `Suspend`/`Resume`; the only "step"
machinery is the ArrowLoop `{state}`/`{out}` protocol). `handle`/`op`
dispatch is synchronous — the "rest of the body" lives in pending Rust eval
frames, so it cannot be captured as a continuation.

The constraints: no Rust concurrency/scheduling code (user requirement), and
per ADR-0072 the non-determinism should be externalized into a scheduler
over an event stream. libc bindings for sockets/epoll/fcntl/errno already
exist (`handl-packages/generated/libc-c-linux-x86_64/`).

Options considered:

- **Rust backend** (`plugin-lib-concurrent`, threads + mpsc): fastest, but
  violates the no-Rust-scheduling constraint and ADR-0083's direction.
- **VM continuations** (suspend/resume instrs): real `await` ergonomics, but
  kernel surgery; defers everything.
- **libuv/libevent**: callback-based C loops need reverse FFI trampolines
  that do not exist; a poll-ified C shim re-exposes epoll with extra
  vendored code — the "intermediate layer" rejected by ADR-0083.
- **Step arrows over epoll**: tasks are explicit state machines the
  scheduler round-robins. No new infra; epoll bindings already generated.

Sanada's _Algebraic Effects and Handlers for Arrows_ (the calculus's
foundation) makes the step model principled rather than ad hoc: folding a
task arrow into the free A-algebra yields `{op, args, k}` nodes where `k` is
the continuation _as data_ — serializable, inspectable, schedulable. The
explicit `{state}` field is the same defunctionalized continuation written
by hand.

## Decision

Adopt the **step-arrow model**, staged:

- **B0 (now)**: `handl-packages/concurrent/concurrent.hnl` defines the step
  protocol — a task is `S ~> Step` where `Step = {state: S} | {out: V} |
{call: Req, state: S}`; `{call}` suspends on a named op (`name` is the
  discriminator — `op` is a grammar keyword) and the scheduler resumes the
  task by re-invoking the arrow with `state.result = R`.
  `handl-packages/concurrent-epoll/` implements it over the generated libc
  bindings: run queue, spawn/await/`enter` continuation frames,
  rendezvous + bounded channels with select, mutex/rwlock/semaphore/
  atomics, cancellation tokens, monotonic timers, nonblocking
  recv/send/accept/connect_wait/close with epoll parking, EBADF
  wake-on-close, and deadlock reporting. Epoll is created lazily so
  CPU-only workloads never touch it.
- **B1 (later)**: a free-algebra interpreter (à la `arrow_model.hnl`) that
  suspends ordinary arrow code for the user — straight-line `await`
  ergonomics on the same wire protocol, no VM surgery.

No new Rust. HTTP/RFC packages build on this once the backend is verified.

## Consequences

- Users write phase-tagged state machines today (`s.phase` dispatch); the
  ergonomics cost is accepted pending B1.
- `concurrent-epoll` is Linux-only; a kqueue/IOCP backend would be a
  sibling package under the same `concurrent` contract.
- `() ~[1]~>` evaluates its body at module load (it is an application, not
  a binding) — nullary helpers must take a dummy parameter.
- Runtime verification of the backend waits on the in-flight
  import/eval refactor landing (imported exported arrows must be
  applicable; `ffi.call` must dispatch through the FFI layer).

## Status

accepted

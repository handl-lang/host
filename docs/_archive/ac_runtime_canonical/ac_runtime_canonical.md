**ac_runtime_canonical**

# Module: ac_runtime_canonical

## Contents

**Modules**

- [`continuation`](#continuation) - Serializable continuation for resumable evaluation.
- [`eval`](#eval)
- [`handlers`](#handlers)
- [`pattern`](#pattern)
- [`vm`](#vm)

---

## Module: continuation

Serializable continuation for resumable evaluation.

The [`Continuation`] type represents a paused evaluation that can
be serialized, sent to another machine, and resumed with a
different host — true cross-machine resumability without
re-evaluating from scratch.

## How it works

Instead of using Rust's call stack (which is not serializable),
`eval_resumable` maintains an explicit stack of [`Frame`]s. Each
frame records:

- The function name being called.
- Which arguments have been evaluated so far.
- Which arguments still need evaluation.

When a host function returns `Err(Error::Yield(value))`, the
current frame stack is captured into a [`Continuation`]. The
caller can serialize it (it's all JSON internally), ship it to
another machine, and resume by calling `eval_resumable` again
with a fresh [`Vm`] and [`Context`].

## Example

```ignore
// Machine A: start evaluation
let vm = Vm::new(server_host);
let result = vm.eval_resumable(&expr, &mut ctx, None);
match result {
    EvalResult::Yield(cont) => {
        // Serialize and send to Machine B
        let json = cont.to_json();
        send_to_client(json);
    }
    EvalResult::Done(value) => { /* finished on server */ }
    EvalResult::Error(err) => { /* handle error */ }
}

// Machine B: resume evaluation
let cont = Continuation::from_json(&received_json)?;
let vm = Vm::new(client_host);
let result = vm.eval_resumable(&expr, &mut ctx, Some(cont));
```



## Module: eval



## Module: handlers



## Module: pattern



## Module: vm




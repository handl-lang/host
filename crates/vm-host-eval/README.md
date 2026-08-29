# handl-vm-host-eval

Eval host implementation for the handl VM.

## Overview

`EvalHost` is a dual-role host:

- **VM function host** — it implements `handl_plugin_vm::Host<NonFunctionValue>` and
  provides the built-in `var`, `resolve`, and `apply` functions available in all
  handl expressions.
- **Platform capability host** — it implements `handl_platform::host::Host` by
  embedding a `handl_platform::DefaultHost` and delegating to its plugin, kit,
  and extension registries.

This design lets the same `EvalHost` instance drive the VM while also
participating in the platform plugin system.

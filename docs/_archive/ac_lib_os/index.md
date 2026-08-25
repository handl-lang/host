# handl_lib_os

# handl-lib-os

Abstract OS service traits for the handl runtime.

Defines abstract service interfaces that bridge host capabilities
(filesystem, environment, path manipulation, time, command execution, temp files,
sockets, signals)
into callable [`Module`] implementations.

## Architecture

```text
handl-runtime-core          handl-lib-os                     handl-lib-os-std
─────────────            ──────────────────            ─────────────────────
Module trait             Fs trait                      StdFs (std::fs)
ModuleHost               Env trait                     StdEnv (std::env)
                         Path trait                    StdPath (std::path)
                         Time trait                    StdTime (std::time)
                         Cmd trait                     StdCmd (std::process)
                         Tmp trait                     StdTmp (std::temp_dir)
                         Socket trait                  StdSocket (std::net)
                         Sig trait                     StdSig (std::process)
                         OsError
                         *Module adapters
```

- **This crate** — abstract service traits + module adapters + test stubs.
  No I/O, no platform dependencies.
- **`handl-lib-os-std`** — concrete implementations using
  Rust stdlib.

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_utils::{MathModule, StrModule};
use handl_lib_os::{
    FsModule, EnvModule, PathModule, TimeModule,
    CmdModule, TmpModule, SocketModule, SigModule,
    fs::fs_stub::StubFs,
    env::env_stub::StubEnv,
    path::path_stub::StubPath,
    time::time_stub::StubTime,
    cmd::cmd_stub::StubCmd,
    tmp::tmp_stub::StubTmp,
    socket::socket_stub::StubSocket,
    sig::sig_stub::StubSig,
};

let host = ModuleHost::new()
    .with(MathModule)
    .with(StrModule)
    .with(FsModule::new(StubFs::default()))
    .with(EnvModule::new(StubEnv::default()))
    .with(PathModule::new(StubPath::default()))
    .with(TimeModule::new(StubTime::default()))
    .with(CmdModule::new(StubCmd::default()))
    .with(TmpModule::new(StubTmp::default()))
    .with(SocketModule::new(StubSocket::default()))
    .with(SigModule::new(StubSig::default()));
```

Expressions can then call `fs.read("config.yml")`, `env.get("HOME")`,
`path.join("a", "b")`, `time.now()`, `cmd.run("echo", "hello")`, `tmp.file()`,
`socket.connect("addr")`, `sig.send(42, "SIGINT")`, etc.

## Modules

### [`handl_lib_os`](handl_lib_os.md)

*10 modules*

### [`cmd`](cmd.md)

*2 modules*

### [`cmd::cmd`](cmd/cmd.md)

*1 function, 1 trait, 3 structs*

### [`cmd::cmd_stub`](cmd/cmd_stub.md)

*1 struct*

### [`env`](env.md)

*2 modules*

### [`env::env`](env/env.md)

*1 function, 1 trait, 4 structs*

### [`env::env_stub`](env/env_stub.md)

*1 struct*

### [`error`](error.md)

*1 module*

### [`error::error`](error/error.md)

*1 enum*

### [`fs`](fs.md)

*2 modules*

### [`fs::fs`](fs/fs.md)

*1 function, 1 trait, 6 structs*

### [`fs::fs_stub`](fs/fs_stub.md)

*1 struct*

### [`path`](path.md)

*2 modules*

### [`path::path`](path/path.md)

*1 trait, 2 functions, 6 structs*

### [`path::path_stub`](path/path_stub.md)

*1 struct*

### [`process`](process.md)

*2 modules*

### [`process::process`](process/process.md)

*1 function, 1 trait, 6 structs*

### [`process::process_stub`](process/process_stub.md)

*2 structs*

### [`sig`](sig.md)

*2 modules*

### [`sig::sig`](sig/sig.md)

*1 trait, 3 structs*

### [`sig::sig_stub`](sig/sig_stub.md)

*1 struct*

### [`socket`](socket.md)

*3 modules*

### [`socket::async_socket`](socket/async_socket.md)

*1 trait, 1 type alias, 2 functions, 5 structs*

### [`socket::socket`](socket/socket.md)

*1 trait, 2 functions, 5 structs*

### [`socket::socket_stub`](socket/socket_stub.md)

*2 structs*

### [`time`](time.md)

*2 modules*

### [`time::time`](time/time.md)

*1 trait, 3 structs*

### [`time::time_stub`](time/time_stub.md)

*1 struct*

### [`tmp`](tmp.md)

*2 modules*

### [`tmp::tmp`](tmp/tmp.md)

*1 trait, 3 structs*

### [`tmp::tmp_stub`](tmp/tmp_stub.md)

*1 struct*


# handl_lib_os_std

# handl-lib-os-std

Concrete OS service implementations backed by the Rust stdlib.

Provides [`StdFs`] (`std::fs`), [`StdEnv`] (`std::env`),
[`StdPath`] (`std::path`), [`StdTime`] (`std::time`),
[`StdCmd`] (`std::process`), [`StdProcess`] (`std::process`),
[`StdTmp`] (`std::temp_dir`), [`StdSocket`] (`std::net`), [`StdSig`] (platform signals).

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_os::{FsModule, EnvModule, PathModule, TimeModule, CmdModule, ProcessModule, TmpModule, SocketModule, SigModule};
use handl_lib_os_std::{StdFs, StdEnv, StdPath, StdTime, StdCmd, StdProcess, StdTmp, StdSocket, StdSig};

let host = ModuleHost::new()
    .with(FsModule::new(StdFs))
    .with(EnvModule::new(StdEnv))
    .with(PathModule::new(StdPath))
    .with(TimeModule::new(StdTime))
    .with(CmdModule::new(StdCmd))
    .with(ProcessModule::new(StdProcess))
    .with(TmpModule::new(StdTmp))
    .with(SocketModule::new(StdSocket))
    .with(SigModule::new(StdSig));
```

## Modules

### [`handl_lib_os_std`](handl_lib_os_std.md)

*10 modules*

### [`cmd`](cmd.md)

*1 struct*

### [`env`](env.md)

*1 struct*

### [`fs`](fs.md)

*1 struct*

### [`mio_socket`](mio_socket.md)

*1 function, 2 structs*

### [`path`](path.md)

*1 struct*

### [`process`](process.md)

*1 function, 1 struct*

### [`sig`](sig.md)

*1 struct, 2 functions*

### [`socket`](socket.md)

*1 constant, 1 function, 1 struct*

### [`time`](time.md)

*1 struct*

### [`tmp`](tmp.md)

*1 function, 1 struct*


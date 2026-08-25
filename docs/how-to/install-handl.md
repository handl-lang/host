# Install hnl

This guide explains how to build and install the `hnl` runtime from the platform repository.

## Build from source

The hnl runtime is implemented in Rust and lives in the `crates/` directory of the platform repository.

### Requirements

- A recent stable Rust toolchain (1.80 or newer).
- `cargo`.

### Steps

1. Clone the repository:

   ```sh
   git clone https://github.com/handl-lang/platform
   cd platform
   ```

2. Build the CLI:

   ```sh
   cargo build --release -p handl-host-cli
   ```

3. The `hnl` binary is now available at `target/release/hnl`.

## Verify the installation

Run a small expression:

```sh
./target/release/hnl run --expr '"hello"'
```

You should see:

```
"hello"
```

## Add to PATH

For convenience, copy or symlink the binary to a location on your `PATH`:

```sh
ln -s $(pwd)/target/release/hnl ~/.local/bin/hnl
```

## Next steps

- [Write a handl script](./write-a-script)
- [Getting started tutorial](../tutorials/getting-started)

# Agent Notes

## Workspace build and test commands

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo check -p handl-platform`
- `cargo test -p handl-platform`
- `cargo check -p handl-platform-tests`
- `cargo test -p handl-platform-tests`

## Architecture reference

- ADR 0031: `project/adrs/0031-platform-abstractions.md`

## Running the CLI and focused platform tests

- `cargo run --bin hnl -- <command> [args...]`
- `cargo test -p handl-platform -p handl-cli -p handl-capability-tests`

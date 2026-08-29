# `handl-lib-ws-whatwg-wasm` browser smoke test

This is a manual end-to-end smoke test using Playwright + Chromium.

It now builds `handl-lib-ws-whatwg-wasm` with `wasm-pack`, loads the generated
wasm module in the browser, and drives the `ws_whatwg_wasm` runtime functions
(`connect`, `send`, `recv`, `close`) through `wasm-bindgen` exports.

## Run

```bash
# from this directory
npm install
bun run build-wasm
PLAYWRIGHT_BROWSERS_PATH=$HOME/.playwright npx playwright test
```

`wasm-pack` must be installed (`cargo install wasm-pack`).

Chromium was installed to `$HOME/.playwright` from the global `playwright install`.

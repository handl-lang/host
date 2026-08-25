# `handl-lib-ws-whatwg-wasm` browser smoke test

This is a manual end-to-end smoke test using Playwright + Chromium.

It currently validates the browser `WebSocket` environment. To test the real
`handl-lib-ws-whatwg-wasm` module, this needs to be extended to load the
handl `wasm32` runtime (`loader-wasm` / `loader-web`) and run a handl
expression that calls `ws_whatwg_wasm`.

## Run

```bash
# from this directory
npm install
PLAYWRIGHT_BROWSERS_PATH=$HOME/.playwright npx playwright test
```

Chromium was installed to `$HOME/.playwright` from the global `playwright install`.

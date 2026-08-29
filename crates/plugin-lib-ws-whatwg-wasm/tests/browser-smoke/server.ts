import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const pkgDir = new URL('./pkg', import.meta.url).pathname;

const indexHtml = `<!DOCTYPE html>
<html>
  <head>
    <title>WebSocket WHATWG smoke</title>
  </head>
  <body>
    <div id="result">pending</div>
    <script type="module">
      import init, {
        ws_whatwg_wasm_connect,
        ws_whatwg_wasm_send,
        ws_whatwg_wasm_recv,
        ws_whatwg_wasm_close,
      } from './pkg/handl_lib_ws_whatwg_wasm.js';

      async function run() {
        await init();
        try {
          const conn = JSON.parse(ws_whatwg_wasm_connect('ws://localhost:8787/ws'));
          const id = conn.connection_id;

          ws_whatwg_wasm_send(id, JSON.stringify('hello'));

          const raw = await new Promise((resolve, reject) => {
            const deadline = Date.now() + 5000;
            const timer = setInterval(() => {
              const r = ws_whatwg_wasm_recv(id);
              if (r !== 'null') {
                clearInterval(timer);
                resolve(r);
              } else if (Date.now() > deadline) {
                clearInterval(timer);
                reject(new Error('timeout waiting for websocket reply'));
              }
            }, 50);
          });

          const reply = JSON.parse(raw);
          document.getElementById('result').textContent = reply.text ?? raw;
          ws_whatwg_wasm_close(id);
        } catch (e) {
          document.getElementById('result').textContent = 'error: ' + e;
        }
      }
      run();
    </script>
  </body>
</html>`;

Bun.serve({
  port: 8787,
  fetch(req, server) {
    const url = new URL(req.url);
    if (url.pathname === '/ws') {
      if (server.upgrade(req)) return;
      return new Response('WebSocket upgrade failed', { status: 400 });
    }
    if (url.pathname.startsWith('/pkg/')) {
      const file = join(pkgDir, url.pathname.slice('/pkg/'.length));
      if (!existsSync(file)) {
        return new Response('not found', { status: 404 });
      }
      const data = readFileSync(file);
      const contentType = file.endsWith('.wasm')
        ? 'application/wasm'
        : 'text/javascript';
      return new Response(data, { headers: { 'Content-Type': contentType } });
    }
    return new Response(indexHtml, {
      headers: { 'Content-Type': 'text/html' },
    });
  },
  websocket: {
    open() {},
    message(ws, message: string) {
      ws.send(message);
    },
  },
});

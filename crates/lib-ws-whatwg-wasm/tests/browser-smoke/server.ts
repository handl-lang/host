const indexHtml = `<!DOCTYPE html>
<html>
  <head>
    <title>WebSocket WHATWG smoke</title>
  </head>
  <body>
    <div id="result">pending</div>
    <script>
      const ws = new WebSocket('ws://localhost:8787/ws');
      ws.onopen = () => ws.send('hello');
      ws.onmessage = (e) => {
        document.getElementById('result').textContent = e.data;
        ws.close();
      };
      ws.onerror = () => {
        document.getElementById('result').textContent = 'error';
      };
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

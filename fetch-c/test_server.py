"""Canned HTTP fixture for fetch-c.test.hnl.

Serves 127.0.0.1:18347 until it gets GET /done (or a 10-minute deadline):
  GET  /       -> 200 JSON {"hello":"handl"} with X-Test header
  POST /echo   -> 200 JSON echoing the request body
  GET  /done   -> 200 and shutdown
"""

import os
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = 18347
done = False


class H(BaseHTTPRequestHandler):
    def _ok(self, body):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("X-Test", "yes")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        global done
        if self.path == "/done":
            self._ok(b'{"bye":true}')
            done = True
        else:
            self._ok(b'{"hello":"handl"}')

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        self._ok(b'{"echo":"' + self.rfile.read(n) + b'"}')

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    srv = HTTPServer(("127.0.0.1", PORT), H)
    srv.timeout = 30
    deadline = time.time() + 600
    open("/tmp/fetch-c-test-ready", "w").write("1")
    try:
        while not done and time.time() < deadline:
            srv.handle_request()
    finally:
        try:
            os.remove("/tmp/fetch-c-test-ready")
        except OSError:
            pass

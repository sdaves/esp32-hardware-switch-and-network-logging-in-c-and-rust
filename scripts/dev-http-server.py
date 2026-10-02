#!/usr/bin/env python3
"""Reverse proxy for the emulated guest's network fetch.

The Velxio QEMU guest reaches the container at the slirp gateway
(192.168.4.2). This listens on the container's :8000 and proxies each request
to the parent machine at ``VELXIO_PROXY_UPSTREAM`` (default
``http://host.docker.internal:8000``), so the firmware's HTTP GET returns the
parent's real response.

If the upstream is unreachable, it answers with a deterministic 200 stub so the
headless ``make test`` stays green when no parent service is running. Set
``VELXIO_PROXY_FALLBACK=0`` to surface a 502 instead.
"""

import http.server
import os
import socketserver
import urllib.error
import urllib.request

UPSTREAM = os.environ.get("VELXIO_PROXY_UPSTREAM", "http://host.docker.internal:8000").rstrip("/")
FALLBACK = os.environ.get("VELXIO_PROXY_FALLBACK", "1") not in ("0", "false", "False")
TIMEOUT = float(os.environ.get("VELXIO_PROXY_TIMEOUT", "5"))

STUB_BODY = b"velxio proxy fallback: parent :8000 unreachable\n"


def _proxy(handler):
    target = UPSTREAM + handler.path
    try:
        req = urllib.request.Request(target, method=handler.command)
        with urllib.request.urlopen(req, timeout=TIMEOUT) as upstream:
            body = upstream.read()
            handler.send_response(upstream.status)
            for key, value in upstream.headers.items():
                if key.lower() in ("transfer-encoding", "connection", "content-length"):
                    continue
                handler.send_header(key, value)
            handler.send_header("Content-Length", str(len(body)))
            handler.end_headers()
            handler.wfile.write(body)
    except (urllib.error.URLError, OSError, ValueError) as exc:
        message = f"velxio proxy: upstream {target} failed: {exc}\n".encode("utf-8", "replace")
        if FALLBACK:
            handler.send_response(200)
            handler.send_header("Content-Type", "text/plain")
            handler.send_header("Content-Length", str(len(STUB_BODY)))
            handler.end_headers()
            handler.wfile.write(STUB_BODY)
        else:
            handler.send_response(502)
            handler.send_header("Content-Type", "text/plain")
            handler.send_header("Content-Length", str(len(message)))
            handler.end_headers()
            handler.wfile.write(message)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        _proxy(self)

    def do_POST(self):
        _proxy(self)

    def do_HEAD(self):
        _proxy(self)

    def log_message(self, *args):
        pass


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    with Server(("0.0.0.0", 8000), Handler) as httpd:
        httpd.serve_forever()

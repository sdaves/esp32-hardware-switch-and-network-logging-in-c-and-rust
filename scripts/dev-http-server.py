#!/usr/bin/env python3
"""Tiny HTTP server for the emulated guest's network fetch.

The Velxio QEMU guest reaches the container at the slirp gateway
(192.168.4.2) — run this in the container so the firmware's HTTP GET has a
real endpoint to hit. Serves a fixed 200 on any path.
"""

import http.server
import socketserver

BODY = b"ESP32 TEA demo endpoint reached over emulated WiFi\n"


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(BODY)))
        self.end_headers()
        self.wfile.write(BODY)

    def log_message(self, *args):
        pass


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    with Server(("0.0.0.0", 8000), Handler) as httpd:
        httpd.serve_forever()

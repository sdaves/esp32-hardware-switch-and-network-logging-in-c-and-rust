#!/usr/bin/env python3
"""Docker compose build/test listener for the esp32simulated Velxio stack.

Runs on the host (no third-party dependencies). Listens on 0.0.0.0:2222 and
runs docker compose commands from the repo root, streaming the combined
stdout/stderr back to the caller so an agent can read the log.

Run from the repo root:
    python3 .devcontainer/docker-build-listener.py

Routes:
    /                 docker compose ps
    /health           echo ok
    /up               docker compose up -d
    /build            docker compose up --build -d
    /build-firmware   docker compose exec velxio scripts/build.sh
    /test             docker compose exec velxio scripts/test.sh
    /logs             docker compose logs --tail=2000 --no-color
    /ps               docker compose ps
    /down             docker compose down
    /stop             docker compose stop
    /exec?cmd=...     arbitrary command inside the velxio container
"""

import os
import shlex
import socketserver
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

TOOLS_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(TOOLS_DIR)
PORT = int(os.environ.get("DBL_PORT", "2222"))
HOST = os.environ.get("DBL_HOST", "0.0.0.0")
SERVICE = os.environ.get("DBL_SERVICE", "velxio")
CAPTURE_TIMEOUT = int(os.environ.get("DBL_TIMEOUT", "600"))

BUILD_LOCK = threading.Lock()

STREAM_ROUTES = {
    "/up": "docker compose up -d",
    "/build": "docker compose up --build -d",
    "/build-firmware": f"docker compose exec -T {SERVICE} bash scripts/build.sh",
    "/test": f"docker compose exec -T {SERVICE} bash scripts/test.sh",
}

CAPTURE_ROUTES = {
    "/": "docker compose ps",
    "/health": "echo ok",
    "/logs": "docker compose logs --tail=2000 --no-color",
    "/ps": "docker compose ps",
    "/down": "docker compose down",
    "/stop": "docker compose stop",
}


def resolve(path, query):
    if path == "/exec":
        command = query.get("cmd", ["bash -lc 'true'"])[0]
        return (f"docker compose exec -T {SERVICE} bash -lc {shlex.quote(command)}", True)
    if path in STREAM_ROUTES:
        return (STREAM_ROUTES[path], True)
    if path in CAPTURE_ROUTES:
        return (CAPTURE_ROUTES[path], False)
    return None


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def do_GET(self):
        parsed = urlparse(self.path)
        route = resolve(parsed.path, parse_qs(parsed.query))
        if route is None:
            self.send_error(404, "no such route")
            return

        command, stream = route
        if not BUILD_LOCK.acquire(blocking=False):
            self.send_response(503)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.end_headers()
            self.wfile.write(b"busy: another build or test is running\n")
            return

        try:
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            if stream:
                self._stream(command)
            else:
                self._capture(command)
        finally:
            BUILD_LOCK.release()

    def _stream(self, command):
        print(f"[listener] stream: {command}", flush=True)
        proc = subprocess.Popen(
            command,
            shell=True,
            cwd=PROJECT_DIR,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        try:
            assert proc.stdout is not None
            for line in proc.stdout:
                self._write(line)
            proc.wait()
            self._write(f"\n[listener] finished (exit {proc.returncode})\n")
        except (BrokenPipeError, ConnectionResetError):
            print("[listener] client disconnected, killing command", flush=True)
            if proc.poll() is None:
                proc.kill()
                proc.wait()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def _capture(self, command):
        print(f"[listener] capture: {command}", flush=True)
        try:
            proc = subprocess.run(
                command,
                shell=True,
                cwd=PROJECT_DIR,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=CAPTURE_TIMEOUT,
            )
        except subprocess.TimeoutExpired as exc:
            output = exc.stdout or ""
            if isinstance(output, bytes):
                output = output.decode("utf-8", "replace")
            self._write(output)
            self._write(f"\n[listener] timed out after {CAPTURE_TIMEOUT}s\n")
            return
        self._write(proc.stdout or "")
        self._write(f"\n[listener] finished (exit {proc.returncode})\n")

    def _write(self, text):
        self.wfile.write(text.encode("utf-8", "replace"))
        self.wfile.flush()

    def log_message(self, fmt, *args):
        sys.stderr.write("[listener] " + (fmt % args) + "\n")


class Server(socketserver.ThreadingMixIn, HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def main():
    if not os.path.exists(os.path.join(PROJECT_DIR, "docker-compose.yaml")):
        print(f"[listener] warning: no docker-compose.yaml in {PROJECT_DIR}", flush=True)
    server = Server((HOST, PORT), Handler)
    print(f"[listener] listening on {HOST}:{PORT} (project: {PROJECT_DIR})", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.shutdown()
        server.server_close()
        print("[listener] stopped", flush=True)


if __name__ == "__main__":
    main()

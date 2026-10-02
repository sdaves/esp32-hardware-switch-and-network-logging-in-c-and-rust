#!/usr/bin/env bash
# Start/stop the reverse proxy the emulated guest fetches.
#
# The Velxio QEMU guest reaches the container at the slirp gateway
# (192.168.4.2). This listens on the container's :8000 and proxies to the
# parent machine (VELXIO_PROXY_UPSTREAM, default host.docker.internal:8000),
# so UC-5's Wi-Fi HTTP GET returns the parent's real response. With
# VELXIO_PROXY_FALLBACK=1 (default) an unreachable parent yields a 200 stub so
# `make test` stays deterministic.
#
#   scripts/dev-http-server.sh start|stop|status
set -euo pipefail
cd "$(dirname "$0")/.."

PIDFILE="${TMPDIR:-/tmp}/velxio-http8000.pid"
LOG="${TMPDIR:-/tmp}/velxio-http8000.log"

alive() {
  [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null
}

start() {
  if alive; then
    echo "dev-http-server: already running (pid $(cat "$PIDFILE"))"
    return 0
  fi
  # `setsid` puts the server in its own session so it is not attached to the
  # caller's stdio/process group. Without this, a server started from
  # `docker compose exec` (e.g. scripts/test.sh) keeps the exec's output pipe
  # open, so the exec never sees EOF and `make test` hangs. stdin from
  # /dev/null and both streams to a file close every inherited descriptor.
  setsid bash -c "echo \$\$ > '$PIDFILE'; exec python3 scripts/dev-http-server.py" \
    </dev/null >"$LOG" 2>&1 &
  # Wait until the pidfile exists and the port actually accepts a connection.
  for _ in $(seq 1 50); do
    if alive && (exec 3<>/dev/tcp/127.0.0.1/8000) 2>/dev/null; then
      exec 3>&- 2>/dev/null || true
      echo "dev-http-server: started (pid $(cat "$PIDFILE")) on :8000"
      return 0
    fi
    sleep 0.2
  done
  echo "dev-http-server: failed to start (see $LOG)" >&2
  return 1
}

stop() {
  if alive; then
    kill "$(cat "$PIDFILE")" 2>/dev/null || true
  fi
  # Also catch a stray instance that lost its pidfile.
  pkill -f "dev-http-server.py" 2>/dev/null || true
  rm -f "$PIDFILE"
  echo "dev-http-server: stopped"
}

status() {
  if alive; then
    echo "dev-http-server: running (pid $(cat "$PIDFILE"))"
  else
    echo "dev-http-server: not running"
    return 1
  fi
}

case "${1:-start}" in
  start) start ;;
  stop) stop ;;
  restart) stop; start ;;
  status) status ;;
  *) echo "usage: $0 start|stop|restart|status" >&2; exit 2 ;;
esac

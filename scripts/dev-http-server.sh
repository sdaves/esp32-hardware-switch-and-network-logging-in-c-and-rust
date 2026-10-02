#!/usr/bin/env bash
# Start/stop the small HTTP endpoint the emulated guest fetches.
#
# The Velxio QEMU guest reaches the container at the slirp gateway
# (192.168.4.2). This server listens on the container's :8000 so UC-5's
# Wi-Fi HTTP GET returns a real response.
#
#   scripts/dev-http-server.sh start|stop|status
set -euo pipefail
cd "$(dirname "$0")/.."

PIDFILE="${TMPDIR:-/tmp}/velxio-http8000.pid"

start() {
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    echo "dev-http-server: already running (pid $(cat "$PIDFILE"))"
    return 0
  fi
  nohup python3 scripts/dev-http-server.py >/tmp/velxio-http8000.log 2>&1 &
  echo $! > "$PIDFILE"
  sleep 0.5
  echo "dev-http-server: started (pid $(cat "$PIDFILE")) on :8000"
}

stop() {
  if [ -f "$PIDFILE" ]; then
    kill "$(cat "$PIDFILE")" 2>/dev/null || true
    rm -f "$PIDFILE"
    echo "dev-http-server: stopped"
  else
    echo "dev-http-server: not running"
  fi
}

status() {
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    echo "dev-http-server: running (pid $(cat "$PIDFILE"))"
  else
    echo "dev-http-server: not running"
    return 1
  fi
}

case "${1:-start}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  *) echo "usage: $0 start|stop|status" >&2; exit 2 ;;
esac

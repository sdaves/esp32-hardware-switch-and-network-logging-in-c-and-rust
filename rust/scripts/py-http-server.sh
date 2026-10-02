#!/usr/bin/env bash
# Start a plain `python3 -m http.server` on :8000, detached so it survives the
# host listener's exec (mirrors scripts/dev-http-server.sh's setsid pattern).
#   scripts/py-http-server.sh <start|stop|status> [dir]
set -euo pipefail

ROOT="${2:-/tmp/uc5root}"
PIDFILE="/tmp/velxio-pyhttp8000.pid"
LOG="/tmp/velxio-pyhttp8000.log"

alive() { [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; }

case "${1:-}" in
  start)
    if alive; then echo "py-http-server: already running (pid $(cat "$PIDFILE"))"; exit 0; fi
    mkdir -p "$ROOT"
    printf 'hello-from-python-http-server\n' >"$ROOT/index.html"
    setsid bash -c "echo \$\$ > '$PIDFILE'; exec python3 -m http.server 8000 --directory '$ROOT'" \
      </dev/null >"$LOG" 2>&1 &
    for _ in $(seq 1 50); do
      if alive && (exec 3<>/dev/tcp/127.0.0.1/8000) 2>/dev/null; then
        exec 3>&- 2>/dev/null || true
        echo "py-http-server: started (pid $(cat "$PIDFILE")) on :8000 root=$ROOT"
        exit 0
      fi
      sleep 0.2
    done
    echo "py-http-server: failed to start (see $LOG)" >&2
    exit 1
    ;;
  stop)
    alive && kill "$(cat "$PIDFILE")" 2>/dev/null || true
    pkill -f "python3 -m http.server" 2>/dev/null || true
    rm -f "$PIDFILE"
    echo "py-http-server: stopped"
    ;;
  status)
    if alive; then echo "py-http-server: running (pid $(cat "$PIDFILE"))"; else echo "py-http-server: not running"; fi
    ;;
  *)
    echo "usage: $0 <start|stop|status> [dir]" >&2; exit 2 ;;
esac

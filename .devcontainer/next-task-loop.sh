#!/usr/bin/env bash
set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INTERVAL="${NEXT_TASK_POLL:-2}"
LOCK_FILE="${TMPDIR:-/tmp}/next-task-loop.lock"

command -v opencode >/dev/null 2>&1 || export PATH="$HOME/.opencode/bin:$PATH"

exec 200>"$LOCK_FILE"
if ! flock -n 200; then
    echo "next-task-loop: already running (lock: $LOCK_FILE)" >&2
    exit 1
fi

run_once() {
    opencode run --command next-task --auto
}

trap 'echo; echo "next-task-loop: stopping"; exit 0' INT TERM
trap 'prev=""' HUP

prev="$(git -C "$REPO_ROOT" status --porcelain | grep -v '\.opencode')"

while true; do
    sleep "$INTERVAL"
    cur="$(git -C "$REPO_ROOT" status --porcelain | grep -v '\.opencode')"
    if [ "$cur" != "$prev" ]; then
        prev="$cur"
        run_once
    fi
done

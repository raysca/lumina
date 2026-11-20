#!/usr/bin/env bash
# Launch Lumina as a Wayland client on nested Weston, then stop it with
# SIGTERM (the package smoke covers the Ctrl+Q quit path).
# Run under xvfb-run: Weston uses its X11 backend to provide a headless seat.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
BIN="${LUMINA_UI_BIN:-${CARGO_TARGET_DIR:-target}/debug/Lumina}"
TIMEOUT_SECS="${LUMINA_WAYLAND_SMOKE_TIMEOUT:-20}"

for tool in weston Xvfb; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "Missing required Wayland smoke tool: $tool" >&2
        exit 1
    }
done
[[ -n "${DISPLAY:-}" ]] || { echo "DISPLAY is required (run under xvfb-run)" >&2; exit 1; }
if [[ -z "${LUMINA_UI_BIN:-}" || ! -x "$BIN" ]]; then
    echo "Building $BIN..."
    cargo build -p lumina-ui-gpui
fi
[[ -x "$BIN" ]] || { echo "Lumina binary not executable: $BIN" >&2; exit 1; }

SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-wayland-smoke.XXXXXX")"
chmod 700 "$SMOKE_HOME"
RUNTIME_DIR="$SMOKE_HOME/runtime"
mkdir -m 700 "$RUNTIME_DIR"
APP_LOG="$SMOKE_HOME/app.log"
WESTON_LOG="$SMOKE_HOME/weston.log"
WESTON_PID=""
APP_PID=""
stop_child() {
    local pid="$1"
    [[ -n "$pid" ]] || return 0
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 25); do
        local state
        state="$(ps -o stat= -p "$pid" 2>/dev/null | tr -d ' ' || true)"
        [[ -z "$state" || "$state" == Z* ]] && break
        sleep 0.2
    done
    local state
    state="$(ps -o stat= -p "$pid" 2>/dev/null | tr -d ' ' || true)"
    if [[ -n "$state" && "$state" != Z* ]]; then
        kill -KILL "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
}
cleanup() {
    stop_child "$APP_PID"
    stop_child "$WESTON_PID"
    rm -rf "$SMOKE_HOME"
}
trap cleanup EXIT

XDG_RUNTIME_DIR="$RUNTIME_DIR" weston \
    --backend=x11 \
    --renderer=pixman \
    --socket=wayland-lumina \
    --width=1280 \
    --height=900 \
    --idle-time=0 \
    --no-config \
    --log="$WESTON_LOG" >"$SMOKE_HOME/weston.out" 2>&1 &
WESTON_PID=$!
SOCKET="$RUNTIME_DIR/wayland-lumina"
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )) && [[ ! -S "$SOCKET" ]]; do
    if ! kill -0 "$WESTON_PID" 2>/dev/null; then
        echo "Weston exited before creating its Wayland socket" >&2
        cat "$WESTON_LOG" >&2 2>/dev/null || true
        cat "$SMOKE_HOME/weston.out" >&2 || true
        exit 1
    fi
    sleep 0.2
done
[[ -S "$SOCKET" ]] || {
    echo "Weston did not create a Wayland socket within ${TIMEOUT_SECS}s" >&2
    cat "$WESTON_LOG" >&2 2>/dev/null || true
    exit 1
}

XDG_RUNTIME_DIR="$RUNTIME_DIR" \
    WAYLAND_DISPLAY=wayland-lumina \
    HOME="$SMOKE_HOME" \
    XDG_CONFIG_HOME="$SMOKE_HOME/config" \
    XDG_DATA_HOME="$SMOKE_HOME/data" \
    XDG_CACHE_HOME="$SMOKE_HOME/cache" \
    env -u DISPLAY "$BIN" >"$APP_LOG" 2>&1 &
APP_PID=$!

READY=""
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    if ! kill -0 "$APP_PID" 2>/dev/null; then
        echo "Lumina exited before opening a Wayland window" >&2
        tail -60 "$APP_LOG" >&2 || true
        exit 1
    fi
    if grep -Fq 'Selected GPU adapter' "$APP_LOG" \
        && grep -Fq 'main window opened' "$APP_LOG"; then
        READY=1
        break
    fi
    sleep 0.2
done
[[ -n "$READY" ]] || {
    echo "Lumina did not create a Wayland window within ${TIMEOUT_SECS}s" >&2
    tail -60 "$APP_LOG" >&2 || true
    cat "$WESTON_LOG" >&2 || true
    exit 1
}

# The nested compositor and the Wayland window have been observed. The
# separate X11/package smoke exercises the normal keyboard quit path; here a
# bounded SIGTERM confirms the Wayland client stops when the runner requests it.
kill -TERM "$APP_PID"
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    PROC_STATE="$(ps -o stat= -p "$APP_PID" 2>/dev/null | tr -d ' ' || true)"
    [[ -z "$PROC_STATE" || "$PROC_STATE" == Z* ]] && break
    sleep 0.2
done
PROC_STATE="$(ps -o stat= -p "$APP_PID" 2>/dev/null | tr -d ' ' || true)"
if [[ -n "$PROC_STATE" && "$PROC_STATE" != Z* ]]; then
    echo "Lumina did not stop after SIGTERM under Wayland" >&2
    tail -60 "$APP_LOG" >&2 || true
    exit 1
fi
wait "$APP_PID" 2>/dev/null || true
APP_PID=""
echo "PASS: Lumina created a Wayland window and shut down on request"

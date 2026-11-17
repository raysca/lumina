#!/usr/bin/env bash
# Launch the installed Debian package under X11 and quit through Ctrl+Q.
set -euo pipefail

BIN="${LUMINA_PACKAGE_BIN:-/usr/bin/lumina}"
TIMEOUT_SECS="${LUMINA_PACKAGE_SMOKE_TIMEOUT:-20}"

for tool in xwininfo xdotool; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "Missing required package smoke tool: $tool" >&2
        exit 1
    }
done
[[ -x "$BIN" ]] || { echo "Installed Lumina binary not executable: $BIN" >&2; exit 1; }
[[ -n "${DISPLAY:-}" ]] || { echo "DISPLAY is required (run under xvfb-run)" >&2; exit 1; }

OWN_SMOKE_HOME=0
if [[ -n "${LUMINA_PACKAGE_SMOKE_HOME:-}" ]]; then
    SMOKE_HOME="$LUMINA_PACKAGE_SMOKE_HOME"
    mkdir -p "$SMOKE_HOME"
else
    SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-package-smoke.XXXXXX")"
    OWN_SMOKE_HOME=1
fi
chmod 700 "$SMOKE_HOME"
mkdir -p "$SMOKE_HOME/user-library"
printf 'keep this user data\n' > "$SMOKE_HOME/user-library/sentinel.txt"
APP_LOG="$SMOKE_HOME/app.log"
APP_PID=""
cleanup() {
    if [[ -n "$APP_PID" ]]; then
        kill "$APP_PID" 2>/dev/null || true
        wait "$APP_PID" 2>/dev/null || true
    fi
    if [[ "$OWN_SMOKE_HOME" == 1 ]]; then
        rm -rf "$SMOKE_HOME"
    fi
}
trap cleanup EXIT

echo "Launching installed package binary: $BIN"
HOME="$SMOKE_HOME" \
    XDG_CONFIG_HOME="$SMOKE_HOME/config" \
    XDG_DATA_HOME="$SMOKE_HOME/data" \
    XDG_CACHE_HOME="$SMOKE_HOME/cache" \
    "$BIN" >"$APP_LOG" 2>&1 &
APP_PID=$!

WINDOW=""
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    if ! kill -0 "$APP_PID" 2>/dev/null; then
        echo "Installed app exited before opening a window" >&2
        tail -60 "$APP_LOG" >&2 || true
        exit 1
    fi
    WINDOW="$(xwininfo -root -tree 2>/dev/null | awk '/Lumina/ {print $1; exit}')"
    [[ -n "$WINDOW" ]] && break
    sleep 0.25
done
[[ -n "$WINDOW" ]] || {
    echo "Installed app did not create a window within ${TIMEOUT_SECS}s" >&2
    tail -60 "$APP_LOG" >&2 || true
    exit 1
}

# Xvfb may report its initial focus target (window 1) while --window sends
# the synthetic key directly to Lumina.
xdotool key --window "$WINDOW" --clearmodifiers ctrl+q 2>/dev/null

DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    PROC_STATE="$(ps -o stat= -p "$APP_PID" 2>/dev/null | tr -d ' ' || true)"
    [[ -z "$PROC_STATE" || "$PROC_STATE" == Z* ]] && break
    sleep 0.25
done
PROC_STATE="$(ps -o stat= -p "$APP_PID" 2>/dev/null | tr -d ' ' || true)"
if [[ -n "$PROC_STATE" && "$PROC_STATE" != Z* ]]; then
    echo "Installed app did not exit through Ctrl+Q" >&2
    tail -60 "$APP_LOG" >&2 || true
    exit 1
fi
wait "$APP_PID"
APP_PID=""

[[ -f "$SMOKE_HOME/config/lumina/config.json" ]] || {
    echo "Normal quit did not persist the app config" >&2
    tail -60 "$APP_LOG" >&2 || true
    exit 1
}
[[ "$(cat "$SMOKE_HOME/user-library/sentinel.txt")" == "keep this user data" ]] || {
    echo "App changed the user data sentinel" >&2
    exit 1
}
echo "PASS: installed package opened, handled Ctrl+Q, and saved config"
echo "PACKAGE_SMOKE_HOME=$SMOKE_HOME"

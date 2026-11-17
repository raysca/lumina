#!/usr/bin/env bash
# Live UI smoke test: launches the real Lumina binary, verifies its window,
# exercises a fixture scan and viewer open on Linux, optionally captures a
# macOS screenshot when Screen Recording permission is available, then quits.
#
# Permission-light and CI-safe:
#   - no Accessibility/System Events usage
#   - screenshot failures are reported as SKIP, not FAIL
#   - hangs are caught by timeouts and the app is always terminated
# A Linux LUMINA_UI_BIN override must be built with --features linux-ui-smoke.
#
# Exit codes: 0 = pass or skip, 1 = failure.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BIN="${LUMINA_UI_BIN:-${CARGO_TARGET_DIR:-target}/debug/lumina-ui-gpui}"
OUT_DIR="${LUMINA_SMOKE_OUT_DIR:-target/ui-smoke}"
TIMEOUT_SECS="${LUMINA_SMOKE_TIMEOUT:-15}"

if [[ "$(uname -s)" == "Linux" ]]; then
    if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
        echo "SKIP: no Linux display (use xvfb-run for a headless smoke test)"
        exit 0
    fi
    if ! command -v xwininfo >/dev/null 2>&1; then
        echo "SKIP: xwininfo not available for X11 window enumeration"
        exit 0
    fi
    # Rebuild the default binary with the smoke hook: target/ may be shared
    # with macOS, where an executable at this path is not runnable by Linux.
    if [[ -z "${LUMINA_UI_BIN:-}" || ! -x "$BIN" ]]; then
        echo "Building $BIN..."
        cargo build -p lumina-ui-gpui --features linux-ui-smoke
    fi

    mkdir -p "$OUT_DIR"
    SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-smoke.XXXXXX")"
    chmod 700 "$SMOKE_HOME"
    SMOKE_LIBRARY="$SMOKE_HOME/library"
    mkdir -p "$SMOKE_LIBRARY" "$SMOKE_HOME/config/lumina"
    cp crates/lumina-ui-gpui/tests/fixtures/clip.mp4 "$SMOKE_LIBRARY/smoke.mp4"
    cat > "$SMOKE_HOME/config/lumina/config.json" <<EOF
{"last_library_path":"$SMOKE_LIBRARY"}
EOF
    APP_LOG="$OUT_DIR/app.log"
    WINDOW_PID=""
    cleanup() {
        if [[ -n "$WINDOW_PID" ]]; then
            kill "$WINDOW_PID" 2>/dev/null || true
            for _ in $(seq 1 25); do
                kill -0 "$WINDOW_PID" 2>/dev/null || break
                sleep 0.2
            done
            kill -9 "$WINDOW_PID" 2>/dev/null || true
        fi
        rm -rf "$SMOKE_HOME"
    }
    trap cleanup EXIT

    echo "Launching $BIN with isolated HOME=$SMOKE_HOME and fixture library"
    HOME="$SMOKE_HOME" XDG_CONFIG_HOME="$SMOKE_HOME/config" XDG_DATA_HOME="$SMOKE_HOME/data" \
        XDG_CACHE_HOME="$SMOKE_HOME/cache" LUMINA_UI_SMOKE=1 \
        "$BIN" >"$APP_LOG" 2>&1 &
    WINDOW_PID=$!
    FOUND=""
    VIEWER_READY=""
    DEADLINE=$((SECONDS + TIMEOUT_SECS))
    while (( SECONDS < DEADLINE )); do
        if ! kill -0 "$WINDOW_PID" 2>/dev/null; then
            echo "FAIL: app process exited before creating a window"
            tail -40 "$APP_LOG" || true
            exit 1
        fi
        FOUND="$(xwininfo -root -tree 2>/dev/null | awk '$0 ~ /Lumina/ {print; exit}')"
        if [[ -n "$FOUND" ]] \
            && grep -Fq "LUMINA_UI_SMOKE: viewer frame loaded" "$APP_LOG" \
            && grep -Fq "LUMINA_UI_SMOKE: video player loaded" "$APP_LOG"; then
            VIEWER_READY=1
            break
        fi
        sleep 0.4
    done
    if [[ -z "$FOUND" ]]; then
        echo "FAIL: no Lumina window within $TIMEOUT_SECS s"
        tail -40 "$APP_LOG" || true
        exit 1
    fi
    echo "PASS: Lumina window appeared: $FOUND"
    if [[ -z "$VIEWER_READY" ]]; then
        echo "FAIL: the fixture video did not load a viewer frame and GStreamer player"
        if [[ -n "${LUMINA_UI_BIN:-}" ]]; then
            echo "An overridden Linux binary must be built with --features linux-ui-smoke"
        fi
        tail -40 "$APP_LOG" || true
        exit 1
    fi
    grep -F "LUMINA_UI_SMOKE: scan complete with 1 media items" "$APP_LOG" >/dev/null || {
        echo "FAIL: the fixture scan did not report its one media item"
        tail -40 "$APP_LOG" || true
        exit 1
    }
    echo "PASS: fixture library scanned and its video opened in the viewer and player"
    if command -v xdotool >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
        WINDOW_ID="$(awk '{print $1}' <<<"$FOUND")"
        WINDOW_ID_DECIMAL="$((WINDOW_ID))"
        CATALOG_DIR="$SMOKE_HOME/data/lumina"
        xdotool windowfocus --sync "$WINDOW_ID"
        FOCUSED_WINDOW="$(xdotool getwindowfocus)"
        [[ "$FOCUSED_WINDOW" == "$WINDOW_ID_DECIMAL" ]] || {
            echo "FAIL: Lumina did not take keyboard focus (window $WINDOW_ID_DECIMAL, focused $FOCUSED_WINDOW)"
            exit 1
        }
        wait_for_favorite() {
            python3 - "$CATALOG_DIR" "$1" <<'PY'
import glob
import os
import sqlite3
import sys
import time

catalog_dir, expected = sys.argv[1], int(sys.argv[2])
deadline = time.monotonic() + 8
while time.monotonic() < deadline:
    databases = glob.glob(os.path.join(catalog_dir, "catalog_*.db"))
    if not databases:
        time.sleep(0.1)
        continue
    connection = sqlite3.connect(f"file:{databases[0]}?mode=ro", uri=True, timeout=2)
    connection.execute("PRAGMA busy_timeout=2000")
    try:
        value = connection.execute("SELECT is_favorite FROM media_items").fetchone()
    except sqlite3.OperationalError:
        value = None
    connection.close()
    if value is not None and value[0] == expected:
        raise SystemExit(0)
    time.sleep(0.1)
raise SystemExit(1)
PY
        }
        xdotool key --clearmodifiers f
        wait_for_favorite 1 || {
            echo "FAIL: the focused viewer did not persist the F favorite shortcut"
            tail -40 "$APP_LOG" || true
            exit 1
        }
        xdotool key --clearmodifiers f
        wait_for_favorite 0 || {
            echo "FAIL: the focused viewer did not persist the second F shortcut"
            tail -40 "$APP_LOG" || true
            exit 1
        }
        echo "PASS: focused Linux window handled F and persisted favorite changes"
    else
        echo "SKIP: xdotool or python3 unavailable; Linux keyboard integration was not checked"
    fi
    kill "$WINDOW_PID" 2>/dev/null || true
    for _ in $(seq 1 25); do
        PROC_STATE="$(ps -o stat= -p "$WINDOW_PID" 2>/dev/null | tr -d ' ' || true)"
        [[ -z "$PROC_STATE" || "$PROC_STATE" == Z* ]] && break
        sleep 0.2
    done
    PROC_STATE="$(ps -o stat= -p "$WINDOW_PID" 2>/dev/null | tr -d ' ' || true)"
    if [[ -n "$PROC_STATE" && "$PROC_STATE" != Z* ]]; then
        echo "FAIL: app did not exit after SIGTERM"
        kill -9 "$WINDOW_PID" 2>/dev/null || true
        wait "$WINDOW_PID" 2>/dev/null || true
        WINDOW_PID=""
        exit 1
    fi
    wait "$WINDOW_PID" 2>/dev/null || true
    WINDOW_PID=""
    echo "PASS: Linux live UI smoke"
    exit 0
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "SKIP: live UI smoke supports macOS and Linux"
    exit 0
fi
if ! command -v swift >/dev/null 2>&1; then
    echo "SKIP: swift not available for CoreGraphics window enumeration"
    exit 0
fi
if ! pgrep -x WindowServer >/dev/null 2>&1; then
    echo "SKIP: no WindowServer (headless session)"
    exit 0
fi
if [[ ! -x "$BIN" ]]; then
    echo "Building $BIN..."
    cargo build -p lumina-ui-gpui
fi

mkdir -p "$OUT_DIR"
SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-smoke.XXXXXX")"
APP_LOG="$OUT_DIR/app.log"
WINDOW_PID=""

cleanup() {
    if [[ -n "$WINDOW_PID" ]]; then
        kill "$WINDOW_PID" 2>/dev/null || true
        for _ in $(seq 1 20); do
            kill -0 "$WINDOW_PID" 2>/dev/null || break
            sleep 0.2
        done
        kill -9 "$WINDOW_PID" 2>/dev/null || true
    fi
    rm -rf "$SMOKE_HOME"
}
trap cleanup EXIT

cat > "$OUT_DIR/window_probe.swift" <<'SWIFT'
import CoreGraphics
import Foundation

guard CommandLine.arguments.count > 1, let target = Int(CommandLine.arguments[1]) else {
    print("COUNT 0")
    exit(0)
}
let windows = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
var matches: [(Int, Int, Int, Int, Int)] = []
for w in windows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int, pid == target,
          let layer = w[kCGWindowLayer as String] as? Int, layer == 0,
          let number = w[kCGWindowNumber as String] as? Int,
          let bounds = w[kCGWindowBounds as String] as? [String: Any]
    else { continue }
    let width = (bounds["Width"] as? NSNumber)?.intValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.intValue ?? 0
    let x = (bounds["X"] as? NSNumber)?.intValue ?? 0
    guard width >= 400, height >= 300 else { continue }
    matches.append((width * height, number, width, height, x))
}
matches.sort { $0.0 > $1.0 }
for (_, number, width, height, x) in matches {
    print("WINDOW \(number) \(width)x\(height)@\(x)")
}
print("COUNT \(matches.count)")
SWIFT

echo "Launching $BIN with isolated HOME=$SMOKE_HOME"
HOME="$SMOKE_HOME" "$BIN" >"$APP_LOG" 2>&1 &
WINDOW_PID=$!

FOUND=""
COUNT=""
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    if ! kill -0 "$WINDOW_PID" 2>/dev/null; then
        echo "FAIL: app process exited before creating a window"
        echo "--- app log ---"; tail -40 "$APP_LOG" || true
        exit 1
    fi
    PROBE="$(swift "$OUT_DIR/window_probe.swift" "$WINDOW_PID" 2>/dev/null || true)"
    COUNT="$(printf '%s\n' "$PROBE" | awk '/^COUNT /{print $2}' | tail -1)"
    if [[ "$COUNT" -gt 0 ]] 2>/dev/null; then
        FOUND="$(printf '%s\n' "$PROBE" | awk '/^WINDOW /{print; exit}')"
        break
    fi
    sleep 0.4
done

if [[ -z "$FOUND" ]]; then
    echo "FAIL: no on-screen window for pid $WINDOW_PID within $TIMEOUT_SECS s"
    echo "--- app log ---"; tail -40 "$APP_LOG" || true
    exit 1
fi

WINDOW_ID="$(printf '%s' "$FOUND" | awk '{print $2}')"
WINDOW_SIZE="$(printf '%s' "$FOUND" | awk '{print $3}')"
echo "PASS: window $WINDOW_ID ($WINDOW_SIZE) appeared"

if screencapture -x -l "$WINDOW_ID" "$OUT_DIR/window.png" 2>/dev/null && [[ -s "$OUT_DIR/window.png" ]]; then
    echo "PASS: screenshot saved to $OUT_DIR/window.png"
else
    echo "SKIP: screenshot unavailable (Screen Recording permission)"
fi

kill "$WINDOW_PID" 2>/dev/null || true
for _ in $(seq 1 25); do
    kill -0 "$WINDOW_PID" 2>/dev/null || break
    sleep 0.2
done
if kill -0 "$WINDOW_PID" 2>/dev/null; then
    echo "WARN: app did not exit after SIGTERM; sending SIGKILL"
fi
WINDOW_PID=""
echo "PASS: live UI smoke"

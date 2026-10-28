#!/usr/bin/env bash
# Live UI smoke test: launches the real Lumina binary, verifies that a native
# window appears (via CoreGraphics window enumeration), optionally captures a
# screenshot when Screen Recording permission is available, and shuts it down.
#
# Permission-light and CI-safe:
#   - no Accessibility/System Events usage
#   - screenshot failures are reported as SKIP, not FAIL
#   - hangs are caught by timeouts and the app is always terminated
#
# Exit codes: 0 = pass or skip, 1 = failure.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BIN="${LUMINA_UI_BIN:-target/debug/lumina-ui-gpui}"
OUT_DIR="${LUMINA_SMOKE_OUT_DIR:-target/ui-smoke}"
TIMEOUT_SECS="${LUMINA_SMOKE_TIMEOUT:-15}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "SKIP: live UI smoke requires macOS"
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

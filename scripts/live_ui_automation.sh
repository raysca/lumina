#!/usr/bin/env bash
# Real-window UI automation for Lumina (macOS).
#
# Launches the actual binary with an isolated HOME + seeded library, waits for
# the native window, then drives it with OS-level keyboard events (CGEvent) and
# verifies outcomes through screenshots, config.json and the catalog DB.
#
# Accessibility ("PostEvent") permission is required for synthetic input.
# Without it, launch/window/screenshot/config/DB checks still run and the
# interaction tests are reported as SKIP.
#
# Exit: 0 = all runnable checks pass (or skipped), 1 = failure.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BIN="${LUMINA_UI_BIN:-target/debug/lumina-ui-gpui}"
OUT_DIR="${LUMINA_E2E_OUT_DIR:-target/ui-e2e}"
TIMEOUT_SECS="${LUMINA_E2E_TIMEOUT:-20}"
REQUIRE_PERMS="${LUMINA_REQUIRE_UI_PERMISSIONS:-0}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "SKIP: live UI automation requires macOS"
    exit 0
fi
if ! command -v swift >/dev/null 2>&1; then
    echo "SKIP: swift is required for CoreGraphics helpers"
    exit 0
fi
if ! pgrep -x WindowServer >/dev/null 2>&1; then
    echo "SKIP: no WindowServer (headless session)"
    exit 0
fi

mkdir -p "$OUT_DIR"

# --- CoreGraphics helpers -------------------------------------------------
cat > "$OUT_DIR/perm.swift" <<'SWIFT'
import CoreGraphics
print(CGPreflightPostEventAccess())
SWIFT

cat > "$OUT_DIR/window_probe.swift" <<'SWIFT'
import CoreGraphics
import Foundation
guard CommandLine.arguments.count > 1, let target = Int(CommandLine.arguments[1]) else {
    print("COUNT 0")
    exit(0)
}
let windows = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
var matches: [(Int, Int, Int, Int, Int, Int)] = []
for w in windows {
    guard let pid = w[kCGWindowOwnerPID as String] as? Int, pid == target,
          let layer = w[kCGWindowLayer as String] as? Int, layer == 0,
          let number = w[kCGWindowNumber as String] as? Int,
          let bounds = w[kCGWindowBounds as String] as? [String: Any]
    else { continue }
    let width = (bounds["Width"] as? NSNumber)?.intValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.intValue ?? 0
    let x = (bounds["X"] as? NSNumber)?.intValue ?? 0
    let y = (bounds["Y"] as? NSNumber)?.intValue ?? 0
    guard width >= 400, height >= 300 else { continue }
    matches.append((width * height, number, width, height, x, y))
}
matches.sort { $0.0 > $1.0 }
for (_, number, width, height, x, y) in matches {
    print("WINDOW \(number) \(width)x\(height)@\(x),\(y)")
}
print("COUNT \(matches.count)")
SWIFT

cat > "$OUT_DIR/key_post.swift" <<'SWIFT'
import Cocoa
import CoreGraphics
import Foundation
guard CommandLine.arguments.count >= 3, let pid = pid_t(CommandLine.arguments[1]) else { exit(2) }
let name = CommandLine.arguments[2]
let modifier = CommandLine.arguments.count > 3 ? CommandLine.arguments[3] : ""
let keycodes: [String: CGKeyCode] = [
    "i": 34, "f": 3, "escape": 53, "home": 115, "space": 49,
    "right": 124, "left": 123, "slash": 44, "p": 35, "n": 45, "return": 36
]
guard let code = keycodes[name] else { exit(3) }
NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows])
usleep(250_000)
let source = CGEventSource(stateID: .hidSystemState)
for pressed in [true, false] {
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: pressed) else { exit(4) }
    // Modifiers combine with "+", e.g. "alt+cmd".
    var flags: CGEventFlags = []
    if modifier.contains("cmd") { flags.insert(.maskCommand) }
    if modifier.contains("alt") { flags.insert(.maskAlternate) }
    event.flags = flags
    event.postToPid(pid)
}
print("posted \(name)")
SWIFT

# Counts the app's on-screen windows, including the small PiP window that the
# main-window probe filters out.
cat > "$OUT_DIR/window_count.swift" <<'SWIFT'
import CoreGraphics
import Foundation
guard CommandLine.arguments.count > 1, let target = Int(CommandLine.arguments[1]) else {
    print(0)
    exit(0)
}
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
let count = windows.filter { w in
    guard let pid = w[kCGWindowOwnerPID as String] as? Int, pid == target,
          let layer = w[kCGWindowLayer as String] as? Int, layer <= 3,
          let bounds = w[kCGWindowBounds as String] as? [String: Any],
          let width = (bounds["Width"] as? NSNumber)?.intValue,
          let height = (bounds["Height"] as? NSNumber)?.intValue
    else { return false }
    return width >= 200 && height >= 150
}.count
print(count)
SWIFT

# --- helpers --------------------------------------------------------------
CONF_DIR=""
CONF=""
APP_PID=""
SMOKE_HOME=""
WINDOW_ID=""

cleanup() {
    if [[ -n "$APP_PID" ]]; then
        kill "$APP_PID" 2>/dev/null || true
        for _ in $(seq 1 20); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.2; done
        kill -9 "$APP_PID" 2>/dev/null || true
    fi
    if [[ -n "$SMOKE_HOME" ]]; then rm -rf "$SMOKE_HOME"; fi
}
trap cleanup EXIT

fail() { echo "FAIL: $*"; exit 1; }

read_config() {
    python3 - "$CONF" "$1" <<'PY'
import json, sys
try:
    with open(sys.argv[1]) as f:
        value = json.load(f).get(sys.argv[2])
except Exception:
    value = None
print("" if value is None else value)
PY
}

wait_config() {
    local key="$1" expected="$2" deadline=$((SECONDS + TIMEOUT_SECS))
    while (( SECONDS < deadline )); do
        local value; value="$(read_config "$key" || true)"
        if [[ "$value" == "$expected" ]]; then return 0; fi
        sleep 0.25
    done
    return 1
}

catalog_db() {
    find "$SMOKE_HOME/Library/Application Support/lumina" -maxdepth 1 -name "catalog_*.db" 2>/dev/null | head -1
}

db_scalar() {
    local db; db="$(catalog_db)"
    [[ -n "$db" ]] || { echo ""; return 0; }
    python3 - "$db" "$1" <<'PY'
import sqlite3, sys
try:
    conn = sqlite3.connect(sys.argv[1])
    print(conn.execute(sys.argv[2]).fetchone()[0])
except Exception:
    print("")
PY
}

wait_db() {
    local sql="$1" expected="$2" deadline=$((SECONDS + TIMEOUT_SECS))
    while (( SECONDS < deadline )); do
        local value; value="$(db_scalar "$sql" || true)"
        if [[ "$value" == "$expected" ]]; then return 0; fi
        sleep 0.25
    done
    return 1
}

capture() {
    local name="$1"
    screencapture -x -l "$WINDOW_ID" "$OUT_DIR/$name.png" 2>/dev/null || return 1
    [[ -s "$OUT_DIR/$name.png" ]]
}

changed() { ! cmp -s "$1" "$2"; }

inject() {
    swift "$OUT_DIR/key_post.swift" "$APP_PID" "$1" "${2:-}" >/dev/null 2>&1 || fail "could not inject key $1"
    sleep 0.6
}

wait_windows() {
    local expected="$1" deadline=$((SECONDS + TIMEOUT_SECS))
    while (( SECONDS < deadline )); do
        local count; count="$(swift "$OUT_DIR/window_count.swift" "$APP_PID" 2>/dev/null || echo 0)"
        if [[ "$count" == "$expected" ]]; then return 0; fi
        sleep 0.3
    done
    return 1
}

window_info() {
    swift "$OUT_DIR/window_probe.swift" "$APP_PID" 2>/dev/null || true
}

# --- launch ---------------------------------------------------------------
if [[ ! -x "$BIN" ]]; then
    echo "Building $BIN..."
    cargo build -p lumina-ui-gpui
fi

SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-e2e.XXXXXX")"
LIB_DIR="$SMOKE_HOME/pics"
mkdir -p "$LIB_DIR"
for name in one two three; do cp resources/app_icon.png "$LIB_DIR/$name.png"; done
# A real clip for PiP; newest mtime so date-descending order puts it first.
touch -t 202001010000 "$LIB_DIR"/*.png
cp crates/lumina-ui-gpui/tests/fixtures/clip.mp4 "$LIB_DIR/clip.mp4"
CONF_DIR="$SMOKE_HOME/Library/Application Support/lumina"
mkdir -p "$CONF_DIR"
CONF="$CONF_DIR/config.json"
cat > "$CONF" <<JSON
{"last_library_path":"$LIB_DIR","sidebar_width":230.0,"inspector_width":310.0,
 "sidebar_visible":true,"inspector_visible":false,"layout_mode":"browser",
 "grid_columns":4,"card_fit_mode":true,"window_x":200,"window_y":150,
 "window_width":1280,"window_height":840}
JSON

APP_LOG="$OUT_DIR/app.log"
echo "Launching $BIN with isolated HOME=$SMOKE_HOME"
HOME="$SMOKE_HOME" "$BIN" >"$APP_LOG" 2>&1 &
APP_PID=$!

FOUND=""
DEADLINE=$((SECONDS + TIMEOUT_SECS))
while (( SECONDS < DEADLINE )); do
    kill -0 "$APP_PID" 2>/dev/null || { echo "--- app log ---"; tail -30 "$APP_LOG" || true; fail "app exited before creating a window"; }
    INFO="$(window_info)"
    FOUND="$(printf '%s\n' "$INFO" | awk '/^WINDOW /{print; exit}')"
    [[ -n "$FOUND" ]] && break
    sleep 0.4
done
[[ -n "$FOUND" ]] || fail "no window appeared within $TIMEOUT_SECS s"
WINDOW_ID="$(printf '%s' "$FOUND" | awk '{print $2}')"
WINDOW_SIZE="$(printf '%s' "$FOUND" | awk '{print $3}')"
echo "PASS: window $WINDOW_ID ($WINDOW_SIZE) appeared"

wait_db "SELECT COUNT(*) FROM media_items" "4" || fail "catalog did not index the seeded library"
echo "PASS: catalog indexed 3 seeded images and 1 video"

wait_config "inspector_visible" "False" || true
capture base || fail "could not capture the window"
echo "PASS: screenshot $OUT_DIR/base.png"

# --- accessibility-gated interaction tests --------------------------------
POST_EVENT="$(swift "$OUT_DIR/perm.swift" 2>/dev/null || echo false)"
if [[ "$POST_EVENT" != "true" && "$POST_EVENT" != "1" ]]; then
    echo ""
    echo "SKIP: Accessibility / PostEvent permission is not granted to this process;"
    echo "      synthetic keyboard/mouse input would be discarded by macOS."
    echo "      Grant it in System Settings -> Privacy & Security -> Accessibility"
    echo "      for DeepSeek Harness (and the terminal running this script), then"
    echo "      rerun. Base launch/window/screenshot/catalog checks above passed."
    echo "      Opening the Accessibility pane is harmless if you need it:"
    echo "      open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'"
    if [[ "$REQUIRE_PERMS" == "1" ]]; then
        fail "Accessibility permission required (LUMINA_REQUIRE_UI_PERMISSIONS=1)"
    fi
    echo "PASS: live UI automation (interaction tests skipped)"
    exit 0
fi

# 1. Real keystroke -> app state -> persisted config.
[[ "$(read_config inspector_visible)" == "False" ]] || fail "expected inspector closed at start"
inject i
wait_config "inspector_visible" "True" || fail "keyboard toggle did not persist inspector_visible=true"
echo "PASS: keyboard 'i' opened the inspector (config.json)"
inject i
wait_config "inspector_visible" "False" || fail "second 'i' did not close the inspector"
echo "PASS: keyboard 'i' closed the inspector (config.json)"

# 2. Shortcut cheat sheet -> visible UI change.
capture sheet_before || fail "screenshot failed"
inject slash cmd
capture sheet_open || fail "screenshot failed after cmd-/"
changed "$OUT_DIR/sheet_before.png" "$OUT_DIR/sheet_open.png" || fail "cmd-/ produced no visible change"
echo "PASS: Cmd+/ opened the shortcut sheet (pixel diff)"
inject escape
capture sheet_closed || fail "screenshot failed after escape"
changed "$OUT_DIR/sheet_open.png" "$OUT_DIR/sheet_closed.png" || fail "escape did not dismiss the shortcut sheet"
echo "PASS: Escape dismissed the shortcut sheet (pixel diff)"

# 3. Space opens the viewer over the real window.
capture viewer_before || fail "screenshot failed"
inject space
capture viewer_open || fail "screenshot failed after space"
changed "$OUT_DIR/viewer_before.png" "$OUT_DIR/viewer_open.png" || fail "space produced no visible change"
echo "PASS: Space opened the viewer (pixel diff)"
inject escape
capture viewer_closed || fail "screenshot failed after closing viewer"
changed "$OUT_DIR/viewer_open.png" "$OUT_DIR/viewer_closed.png" || fail "escape did not close the viewer"
echo "PASS: Escape closed the viewer (pixel diff)"

# 4. Picture-in-Picture follows the user around (video sorts first).
wait_windows 1 || fail "expected only the main window before PiP"
inject home
inject p alt
wait_windows 2 || fail "Option-P on the selected video did not open the PiP window"
echo "PASS: Option-P opened the PiP window"
# PiP is a floating panel, which macOS hides while its app is inactive
# unless told otherwise; it must stay up while the user is in another app.
osascript -e 'tell application "Finder" to activate' >/dev/null 2>&1 || fail "could not activate Finder"
sleep 1
wait_windows 2 || fail "the PiP window hid when another app became active"
echo "PASS: PiP stayed on screen with another app active"
inject right
capture pip_viewer_before || fail "screenshot failed"
inject space
capture pip_viewer_open || fail "screenshot failed after space"
changed "$OUT_DIR/pip_viewer_before.png" "$OUT_DIR/pip_viewer_open.png" || fail "space on a photo did not open the viewer while PiP played"
wait_windows 2 || fail "viewing a photo closed the PiP window"
echo "PASS: viewing a photo kept PiP open"
inject escape
inject p alt
wait_windows 1 || fail "Option-P did not close the PiP window"
echo "PASS: Option-P closed the PiP window"

# 5. Keyboard action mutates the catalog database.
inject home
inject f
wait_db "SELECT COUNT(*) FROM media_items WHERE is_favorite = 1" "1" || fail "keyboard favorite did not reach sqlite"
echo "PASS: keyboard 'f' persisted a favorite in catalog.db"

# 6. New Smart Album: Option-Cmd-N, then Cmd-Return saves the default
#    "Favourite is Yes" condition as an untitled album.
inject n alt+cmd
inject return cmd
wait_db "SELECT COUNT(*) FROM smart_albums WHERE title = 'Untitled Smart Album'" "1" \
    || fail "Option-Cmd-N then Cmd-Return did not save a smart album"
echo "PASS: keyboard-built smart album persisted in catalog.db"

echo "PASS: live UI automation"

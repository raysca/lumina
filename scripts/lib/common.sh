#!/usr/bin/env bash
# Helpers shared by Lumina's shell scripts.
#
# Source this file; do not execute it. Callers keep their own `set -euo pipefail`,
# variables and traps; this file only removes copy-paste drift between the
# release, packaging and live-check scripts.

# Repository root, derived from this file so a caller can source it before `cd`.
LUMINA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Prints the package version from a Cargo.toml.
crate_version() {
    sed -n 's/^version = "\([^"]*\)"/\1/p' "$1" | head -1
}

# Prints CFBundleShortVersionString from resources/Info.plist.
# PlistBuddy exists on macOS; the grep fallback keeps Linux packaging working.
plist_version() {
    if command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
        /usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" \
            "${1:-resources/Info.plist}"
    else
        grep -A1 '<key>CFBundleShortVersionString</key>' "${1:-resources/Info.plist}" \
            | sed -n 's/.*<string>\(.*\)<\/string>.*/\1/p'
    fi
}

# Stops `pid`: TERM, up to 4 seconds of grace, then KILL. Always succeeds so a
# cleanup trap cannot turn an already-exited child into a failing script.
stop_pid() {
    local pid="${1:-}"
    [[ -n "$pid" ]] || return 0
    kill "$pid" 2>/dev/null || true
    local i
    for i in $(seq 1 20); do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.2
    done
    kill -9 "$pid" 2>/dev/null || true
    # Reap it when it is our child; harmless for a window we did not spawn.
    wait "$pid" 2>/dev/null || true
    return 0
}

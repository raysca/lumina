.PHONY: all help dev run run-release build build-release test test-verbose bench check lint fmt fmt-fix clean install icon bundle bundle-linux dmg release release-linux notarize ui-smoke ui-e2e

# Keep in sync with crates/*/Cargo.toml and resources/Info.plist
VERSION := 0.2.0

# Terminal colors
CYAN    := \033[36m
GREEN   := \033[32m
YELLOW  := \033[33m
RESET   := \033[0m
BOLD    := \033[1m

## Default target
all: test build

## Display help message
help:
	@echo "$(BOLD)Lumina - Zero-Latency Local-First Media Manager$(RESET)"
	@echo "Usage: make $(CYAN)<target>$(RESET)\n"
	@echo "$(BOLD)Development Targets:$(RESET)"
	@echo "  $(CYAN)dev$(RESET)           Run the Lumina GPUI application in debug mode"
	@echo "  $(CYAN)run$(RESET)           Alias for 'dev'"
	@echo "  $(CYAN)run-release$(RESET)   Run the Lumina GPUI application in optimized release mode"
	@echo ""
	@echo "$(BOLD)Build Targets:$(RESET)"
	@echo "  $(CYAN)build$(RESET)         Build all workspace crates in debug mode"
	@echo "  $(CYAN)build-release$(RESET) Build all workspace crates with full optimizations (--release)"
	@echo "  $(CYAN)check$(RESET)         Fast workspace typecheck (cargo check)"
	@echo "  $(CYAN)clean$(RESET)         Remove build artifacts and target directory"
	@echo ""
	@echo "$(BOLD)Testing & Benchmarking:$(RESET)"
	@echo "  $(CYAN)test$(RESET)          Run all unit and integration tests across the workspace"
	@echo "  $(CYAN)test-verbose$(RESET)  Run all tests with live stdout (--nocapture)"
	@echo "  $(CYAN)bench$(RESET)         Execute performance benchmarks (ingest, seeks, smart albums, scale)"
	@echo ""
	@echo "$(BOLD)Code Quality:$(RESET)"
	@echo "  $(CYAN)lint$(RESET)          Run Clippy linter across workspace"
	@echo "  $(CYAN)fmt$(RESET)           Check code formatting"
	@echo "  $(CYAN)fmt-fix$(RESET)       Automatically format all Rust source files"

## Run GPUI native desktop application in debug mode
dev:
	@test -f "$$LUMINA_YT_DLP" || test -x target/debug/yt-dlp || test -x /opt/homebrew/bin/yt-dlp || test -x /usr/local/bin/yt-dlp || command -v yt-dlp >/dev/null 2>&1 || echo "note: no download helper found; run 'make bundle-deps' to add the pinned one."
	cargo run -p lumina-ui-gpui --bin lumina-ui-gpui

## Alias for dev
run: dev

## Run GPUI native desktop application in release mode
run-release:
	cargo run --release -p lumina-ui-gpui --bin lumina-ui-gpui

## Build all workspace crates in debug mode
build:
	cargo build --workspace

## Build all workspace crates in release mode
build-release:
	cargo build --release --workspace

## Fast typecheck
check:
	cargo check --workspace

## Run all workspace unit and integration tests
test:
	cargo test --workspace

## Run all tests with live console output
test-verbose:
	cargo test --workspace -- --nocapture

## Run performance benchmarks and scale gates
bench:
	cargo test -p lumina-core --test benchmark_test -- --include-ignored --nocapture --test-threads=1

## Launch the real app window and verify it appears (permission-light smoke test)
ui-smoke:
	./scripts/live_ui_smoke.sh

## Drive the real app window with OS-level input (needs Accessibility permission)
ui-e2e:
	./scripts/live_ui_automation.sh

## Lint workspace with Clippy
lint:
	cargo clippy --workspace --all-targets --all-features

## Check code formatting
fmt:
	cargo fmt --all -- --check

## Format code
fmt-fix:
	cargo fmt --all

## Generate macOS Retina App Icon (.icns)
icon:
	cargo run -p lumina-ui-gpui --bin generate-icon --features generate-icon

## Fetch the pinned, checksum-verified yt-dlp and ffmpeg and place them next to the dev binaries
bundle-deps:
	./scripts/fetch_yt_dlp.sh
	./scripts/fetch_ffmpeg.sh
	mkdir -p target/debug target/release
	cp target/deps/yt-dlp target/deps/ffmpeg target/debug/
	cp target/deps/yt-dlp target/deps/ffmpeg target/release/

## Package macOS Application Bundle (universal when targets installed)
bundle:
	./scripts/build_release.sh

## Build distributable DMG + zip with checksums
dmg: release
release:
	./scripts/package_release.sh

# Build a native Debian package (requires Linux and dpkg-deb).
bundle-linux release-linux:
	./scripts/package_linux.sh

## Bundle, sign (Developer ID), notarize + staple, then package
notarize:
	LUMINA_NOTARIZE=1 ./scripts/package_release.sh

## Clean cargo build artifacts
clean:
	cargo clean

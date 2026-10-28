.PHONY: all help dev run run-release build build-release test test-verbose bench check lint fmt fmt-fix clean install icon bundle dmg ui-smoke ui-e2e

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
	cargo test --test benchmark_test -- --nocapture

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

## Package macOS Application Bundle (Lumina.app)
bundle: icon
	@echo "$(BOLD)$(CYAN)Building Lumina.app release bundle...$(RESET)"
	cargo build --release -p lumina-ui-gpui --bin lumina-ui-gpui
	@mkdir -p target/release/bundle/osx/Lumina.app/Contents/MacOS
	@mkdir -p target/release/bundle/osx/Lumina.app/Contents/Resources
	@cp target/release/lumina-ui-gpui target/release/bundle/osx/Lumina.app/Contents/MacOS/lumina-ui-gpui
	@chmod +x target/release/bundle/osx/Lumina.app/Contents/MacOS/lumina-ui-gpui
	@cp resources/Info.plist target/release/bundle/osx/Lumina.app/Contents/Info.plist
	@cp resources/Lumina.icns target/release/bundle/osx/Lumina.app/Contents/Resources/Lumina.icns
	@codesign --force --deep --sign - target/release/bundle/osx/Lumina.app
	@echo "$(GREEN)$(BOLD)✓ Lumina.app bundle created at target/release/bundle/osx/Lumina.app$(RESET)"

## Build distributable macOS Disk Image Installer (.dmg)
dmg: bundle
	@echo "$(BOLD)$(CYAN)Creating distributable DMG installer...$(RESET)"
	@rm -rf target/release/bundle/osx/dmg_stage target/release/bundle/osx/Lumina-v$(VERSION).dmg
	@mkdir -p target/release/bundle/osx/dmg_stage
	@cp -R target/release/bundle/osx/Lumina.app target/release/bundle/osx/dmg_stage/
	@ln -s /Applications target/release/bundle/osx/dmg_stage/Applications
	@hdiutil create -volname "Lumina Photos" -srcfolder target/release/bundle/osx/dmg_stage -ov -format UDZO target/release/bundle/osx/Lumina-v$(VERSION).dmg
	@rm -rf target/release/bundle/osx/dmg_stage
	@echo "$(GREEN)$(BOLD)✓ Lumina DMG created at target/release/bundle/osx/Lumina-v$(VERSION).dmg$(RESET)"

## Clean cargo build artifacts
clean:
	cargo clean

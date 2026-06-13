APP_DIR ?= $(CURDIR)
export PATH := $(HOME)/.cargo/bin:$(PATH)
NPM := env -u npm_config_devdir npm
OLLAMA_MODEL ?= qwen3:4b-instruct-2507-q4_K_M
OLLAMA_DEBUG ?= 1
TAURI_DIR := $(APP_DIR)/src-tauri

.PHONY: help dev build install \
	check check-frontend check-backend \
	lint lint-frontend lint-backend \
	fix fix-frontend fix-backend \
	_compile-backend _test-backend _lint-rust _lint-rust-fix _fmt-rust _fmt-rust-fix \
	clean-cache clean-cache-aggressive clean-all \
	build-macos build-macos-intel build-macos-arm \
	release-macos release-macos-intel release-macos-arm notarize-wait notarize-info

# --- Public command contract ---

help:
	@echo "Development:"
	@echo "  make dev              Run the Tauri development app"
	@echo "  make build            Build the Tauri app"
	@echo "  make install          Install npm dependencies"
	@echo ""
	@echo "Validation:"
	@echo "  make fix              Auto-fix frontend and backend issues"
	@echo "  make lint             Verify lint and formatting without changes"
	@echo "  make check            Run type checks, compilation, lint, and tests"
	@echo "  make fix-frontend     Auto-fix frontend issues only"
	@echo "  make lint-frontend    Verify frontend lint and formatting"
	@echo "  make check-frontend   Run frontend type checks, lint, and formatting"
	@echo "  make fix-backend      Auto-fix Rust formatting and Clippy issues"
	@echo "  make lint-backend     Verify Rust lint and formatting"
	@echo "  make check-backend    Run Rust compile checks, lint, and tests"
	@echo ""
	@echo "Recommended cycles:"
	@echo "  Frontend-only: make fix-frontend && make check-frontend"
	@echo "  Backend-only:  make fix-backend && make check-backend"
	@echo "  Full-stack:    make fix && make check"

# --- App lifecycle ---

dev:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' $(NPM) run tauri dev

build:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' $(NPM) run tauri build

install:
	cd $(APP_DIR) && $(NPM) install

# --- Validation workflows ---

check: check-frontend check-backend

check-frontend:
	cd $(APP_DIR) && $(NPM) run check

check-backend: _compile-backend lint-backend _test-backend

lint: lint-frontend lint-backend

lint-frontend:
	cd $(APP_DIR) && $(NPM) run lint

lint-backend: _lint-rust _fmt-rust

fix: fix-frontend fix-backend

fix-frontend:
	cd $(APP_DIR) && $(NPM) run fix

fix-backend:
	cd $(TAURI_DIR) && cargo fmt
	cd $(TAURI_DIR) && cargo clippy --fix --allow-dirty --allow-staged --all-targets -- -D warnings
	cd $(TAURI_DIR) && cargo fmt

# --- Backend internals ---

_compile-backend:
	cd $(TAURI_DIR) && cargo check

_test-backend:
	cd $(TAURI_DIR) && cargo test

_lint-rust:
	cd $(TAURI_DIR) && cargo clippy --all-targets -- -D warnings

_lint-rust-fix:
	cd $(TAURI_DIR) && cargo clippy --fix --allow-dirty --allow-staged --all-targets -- -D warnings

_fmt-rust:
	cd $(TAURI_DIR) && cargo fmt --check

_fmt-rust-fix:
	cd $(TAURI_DIR) && cargo fmt

# --- Cache cleanup ---

clean-cache:
	@echo "[clean-cache] release builds, incremental cache, frontend output (debug deps kept)"
	cd $(TAURI_DIR) && cargo clean --release
	find $(TAURI_DIR)/target -type d -name incremental -exec rm -rf {} + 2>/dev/null || true
	rm -rf $(APP_DIR)/dist $(APP_DIR)/.svelte-kit $(APP_DIR)/build $(TAURI_DIR)/bundle
	@echo "[clean-cache] done"

clean-cache-aggressive: clean-cache
	@echo "[clean-cache-aggressive] build-script cache + copyosity crate artifacts (third-party deps kept)"
	find $(TAURI_DIR)/target -type d -path '*/debug/build' -exec rm -rf {} + 2>/dev/null || true
	cd $(TAURI_DIR) && cargo clean -p copyosity
	@echo "[clean-cache-aggressive] done"

clean-all:
	@echo "[clean-all] target, node_modules, frontend cache, bundles"
	cd $(TAURI_DIR) && cargo clean
	rm -rf $(APP_DIR)/node_modules $(APP_DIR)/dist $(APP_DIR)/.svelte-kit $(APP_DIR)/build
	rm -rf $(TAURI_DIR)/bundle $(APP_DIR)/.tauri
	rm -f $(APP_DIR)/*.dmg
	@echo "[clean-all] done — run 'make install' before dev/build if node_modules was removed"

# --- macOS release builds ---

build-macos:
	cd $(APP_DIR) && MACOS_ARCH=auto ./scripts/build-macos.sh

build-macos-intel:
	cd $(APP_DIR) && MACOS_ARCH=x86_64 ./scripts/build-macos.sh

build-macos-arm:
	cd $(APP_DIR) && MACOS_ARCH=aarch64 ./scripts/build-macos.sh

release-macos:
	cd $(APP_DIR) && MACOS_ARCH=auto KEYCHAIN_PROFILE='AC_PASSWORD' WAIT_FOR_NOTARIZATION=0 ./scripts/release-macos.sh

release-macos-intel:
	cd $(APP_DIR) && MACOS_ARCH=x86_64 KEYCHAIN_PROFILE='AC_PASSWORD' WAIT_FOR_NOTARIZATION=0 ./scripts/release-macos.sh

release-macos-arm:
	cd $(APP_DIR) && MACOS_ARCH=aarch64 KEYCHAIN_PROFILE='AC_PASSWORD' WAIT_FOR_NOTARIZATION=0 ./scripts/release-macos.sh

notarize-info:
	cd $(APP_DIR) && xcrun notarytool info "$$(cat .last_notarization_id)" --keychain-profile AC_PASSWORD

notarize-wait:
	cd $(APP_DIR) && xcrun notarytool wait "$$(cat .last_notarization_id)" --keychain-profile AC_PASSWORD

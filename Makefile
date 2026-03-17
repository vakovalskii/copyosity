APP_DIR := /Users/v.kovalskii/coplys/copyosity
OLLAMA_MODEL ?= qwen3:4b-instruct-2507-q4_K_M
OLLAMA_DEBUG ?= 1

.PHONY: dev build check release-macos notarize-wait notarize-info

dev:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' npm run tauri dev

build:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' npm run tauri build

check:
	cd $(APP_DIR) && npm run check && cd src-tauri && cargo check

release-macos:
	cd $(APP_DIR) && KEYCHAIN_PROFILE='AC_PASSWORD' WAIT_FOR_NOTARIZATION=0 ./scripts/release-macos.sh

notarize-info:
	cd $(APP_DIR) && xcrun notarytool info "$$(cat .last_notarization_id)" --keychain-profile AC_PASSWORD

notarize-wait:
	cd $(APP_DIR) && xcrun notarytool wait "$$(cat .last_notarization_id)" --keychain-profile AC_PASSWORD

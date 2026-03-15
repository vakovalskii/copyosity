APP_DIR := /Users/v.kovalskii/coplys/copyosity
OLLAMA_MODEL ?= qwen3:4b-instruct-2507-q4_K_M
OLLAMA_DEBUG ?= 1

.PHONY: dev build check

dev:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' npm run tauri dev

build:
	cd $(APP_DIR) && COPYOSITY_OLLAMA_MODEL='$(OLLAMA_MODEL)' COPYOSITY_DEBUG_OLLAMA='$(OLLAMA_DEBUG)' npm run tauri build

check:
	cd $(APP_DIR) && npm run check && cd src-tauri && cargo check

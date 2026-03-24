# Makefile for Kelvin Web Search
# Convenience wrapper around the common kelvin-plugin.sh development commands.
# Run 'make help' to list available targets.

PLUGIN_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
MANIFEST   := $(PLUGIN_DIR)plugin.json
SCRIPTS    := $(PLUGIN_DIR)../scripts

ID      := $(shell jq -er '.id'      $(MANIFEST) 2>/dev/null || echo unknown)
VERSION := $(shell jq -er '.version' $(MANIFEST) 2>/dev/null || echo 0.0.0)
PACKAGE := $(PLUGIN_DIR)dist/$(ID)-$(VERSION).tar.gz

.PHONY: build test pack install smoke clean help

build:          ## Compile WASM and update entrypoint_sha256 in plugin.json
	@bash $(PLUGIN_DIR)build.sh

test: build     ## Validate plugin manifest structure
	@$(SCRIPTS)/kelvin-plugin.sh test --manifest $(MANIFEST)

pack: build     ## Create distributable .tar.gz package in dist/
	@$(SCRIPTS)/kelvin-plugin.sh pack --manifest $(MANIFEST)

install: pack   ## Install plugin into the local Kelvin plugin home
	@$(SCRIPTS)/kelvin-plugin.sh install --package $(PACKAGE)

smoke: build    ## Run a local smoke test
	@$(SCRIPTS)/kelvin-plugin.sh smoke --manifest $(MANIFEST)

clean:          ## Remove build artifacts (target/, dist/, payload/*.wasm)
	@rm -rf $(PLUGIN_DIR)target $(PLUGIN_DIR)dist $(PLUGIN_DIR)payload/*.wasm

help:           ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-10s %s\n", $$1, $$2}'

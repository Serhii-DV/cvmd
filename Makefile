# Default target
.DEFAULT_GOAL := help

.PHONY: help
help: ## Display this help message
	@printf 'Usage:\n  make [target]\n\nTargets:\n'
	@grep -hE '^[a-zA-Z0-9_-]+:.*## .*$$' $(MAKEFILE_LIST) | sort | \
	  awk 'BEGIN {FS = ": ## "}; {printf "  %-15s %s\n", $$1, $$2}'

.PHONY: build
build: ## Build and install cvmd into Cargo's binary directory
	cargo install --path . --locked

# Default target
.DEFAULT_GOAL := help

.PHONY: help
help: ## Display this help message
	@printf 'Usage:\n  make [target]\n\nTargets:\n'
	@grep -hE '^[a-zA-Z0-9_-]+:.*## .*$$' $(MAKEFILE_LIST) | sort | \
	  awk 'BEGIN {FS = ":.*## "}; {printf "  %-15s %s\n", $$1, $$2}'

.PHONY: build
build: ## Build and install cvmd into Cargo's binary directory
	cargo install --path . --locked

.PHONY: build-dev
build-dev: ## Build the development binary without installing it
	cargo build --locked

.PHONY: cli-help
cli-help: ## Display local CLI usage
	cargo run --locked -- --help

.PHONY: init-dev
init-dev: ## Initialize a fresh isolated workspace under /tmp
	@dev_dir=$$(mktemp -d /tmp/cvmd-dev.XXXXXX) && \
	  cargo run --locked -- --config "$$dev_dir/config.toml" init "$$dev_dir/workspace"

.PHONY: fmt
fmt: ## Apply Rust formatting
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## Check Rust formatting
	cargo fmt --all -- --check

.PHONY: lint
lint: ## Run Clippy with warnings treated as errors
	cargo clippy --locked --all-targets -- -D warnings

.PHONY: test
test: ## Run the test suite
	cargo test --locked

.PHONY: check
check: fmt-check lint test ## Run all development checks

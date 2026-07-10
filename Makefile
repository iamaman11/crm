.PHONY: contracts architecture test check
contracts:
	python scripts/validate_contracts.py
architecture:
	python scripts/check_architecture.py
test: architecture
	cargo test --workspace --all-features
check: contracts architecture
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features

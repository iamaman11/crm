.PHONY: contracts architecture test check conformance
contracts:
	python scripts/repo.py contracts
architecture:
	python scripts/repo.py architecture
test: architecture
	cargo test --workspace --all-features
check: contracts architecture
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features
conformance: contracts architecture
	python scripts/repo.py conformance

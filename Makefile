# Orbit — local verification. No CI, no external tools beyond cargo/rustc.
CARGO ?= cargo
PYTHON ?= python3

.PHONY: all build release check test release-test fmt clippy fuzz bench batch browser verify clean

all: build

build:
	$(CARGO) build

release:
	$(CARGO) build --release

check:
	$(CARGO) check --workspace --all-targets

test:
	$(CARGO) test

release-test:
	$(CARGO) test --release

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fuzz:
	$(CARGO) run --release -p orbit-tools --bin orbit-fuzz

bench:
	$(CARGO) run --release -p orbit-tools --bin orbit-bench -- --scale 1

batch:
	$(CARGO) run --release -p orbit-tools --bin orbit-batch

browser: release
	$(PYTHON) tests/playwright_studio.py --binary target/release/orbit

verify: release test release-test
	@echo "Rust verification complete."

clean:
	$(CARGO) clean
	rm -f tests/studio.png

#!/usr/bin/env bash
# Local verification. No CI, no network, no external crates.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== release build =="
cargo build --release

echo "== debug tests =="
cargo test

echo "== release tests =="
cargo test --release

echo "== fuzz (10k random documents + corpus) =="
./target/release/orbit-fuzz --rounds 10000 --corpus fuzz/corpus

echo "== 20 scripted missions =="
./target/release/orbit-batch

echo "Verification complete."

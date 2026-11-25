#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROUNDS="${1:-100000}"
cargo build --release -p orbit-tools
./target/release/orbit-fuzz --rounds "$ROUNDS" --corpus fuzz/corpus

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
SCALE="${1:-1}"
cargo build --release -p orbit-tools
./target/release/orbit-bench --scale "$SCALE"

#!/usr/bin/env bash
# Build and launch the local Studio (opens the browser).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p orbit-studio
exec ./target/release/orbit "$@"

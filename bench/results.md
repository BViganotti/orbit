# Benchmark results

Environment for this run:

- Apple M1 Max, arm64, macOS
- rustc 1.99.1 (58cf8f9c3 2026-10-13)
- `cargo build --release`, `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`
- `orbit-bench --scale 1`

| Benchmark | ms/op | ops/s | Iterations |
|---|---:|---:|---:|
| JSON parse, 55.9 KiB | 0.608 | 1,645 | 500 |
| JSON stringify, small object | <0.001 | 6,458,530 | 500 |
| Atlas ascent | 2.481 | 403 | 10 |
| Atlas full mission | 277.765 | 3.6 | 5 |
| TLI plan, 98 evaluations | 261.319 | 3.8 | 5 |
| 30-day coupled n-body conservation | 12.895 | 77.5 | 2 |
| Dormand–Prince harmonic | 3,731 ksteps/s | — | 98,080 steps |
| RNG u64 | <0.001 | 694,283,409 | 1,000,000 |
| Rocket parse + validate | 0.013 | 76,376 | 2,000 |

All 5 full-mission iterations succeeded. The conservation benchmark executed
35,722 adaptive steps for two 30-day runs (17,861 per run).

## Interpretation

- A full lunar mission costs roughly a quarter of a second in release on this
  machine; most of that is the TLI scan and bisection, each evaluation being a
  real 5-day n-body coast.
- Ascent alone is about 2.5 ms, so the product is not limited by launch
  integration.
- JSON parsing is bounded by document size and is comfortably below a
  millisecond for the 56 KiB test document.
- These numbers are not compared against another implementation and are not a
  claim of optimality.

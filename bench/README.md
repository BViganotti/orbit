# Benchmarks

`orbit-bench` is a standard-library timing harness. It uses `Instant` and
`std::hint::black_box`, with an explicit iteration count and no statistical
post-processing. Results are machine-dependent; the checked-in numbers document
one real run, not a target.

```sh
cargo run --release -p orbit-tools --bin orbit-bench -- --scale 1
# or
./bench/run.sh 1
```

`--scale N` multiplies iteration counts (1–20). Timing includes the full cost of
each operation. Mission and TLI benchmarks include all targeting evaluations,
not just a single integration step.

## What is measured

| Benchmark | Operation |
|---|---|
| `json parse` | parse a 55.9 KiB document with 500 objects and 10,000 numbers |
| `json stringify` | write a small object (allocation included) |
| `atlas ascent` | rotating-Earth ascent with all stages and events |
| `atlas full mission` | ascent, parking, TLI targeting, coast, capture, trim |
| `tli plan` | one 98-evaluation TLI solve from an Atlas parking orbit |
| `nbody 30-day conservation` | coupled Earth–Moon–Sun + test particle integration |
| `dormand-prince harmonic` | adaptive steps on a 100-period oscillator |
| `rng u64` | xorshift128+ word generation |
| `rocket parse` | parse + validate an Atlas JSON object |

## Results

See [results.md](results.md) for the measured table and
[raw.txt](raw.txt) for the unedited harness output used for this release.

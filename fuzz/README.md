# Fuzzing

The fuzz harness is a deterministic std-only binary. It does not require
`cargo-fuzz`, LLVM sanitizers or network access.

```sh
cargo run --release -p orbit-tools --bin orbit-fuzz
# or
./fuzz/run.sh 200000
```

It performs:

1. 100,000 (configurable) random documents from a JSON-heavy alphabet;
2. replay of every `.json` file in `fuzz/corpus`;
3. 2,000 structurally invalid rocket objects;
4. 1,000 random mission requests;
5. 100 randomised ascent configurations.

Every parser call and simulation runs inside `catch_unwind`. Any panic, NaN or
non-finite state increments a failure counter and the process exits non-zero.
The random seed is fixed, so a failure is reproducible.

## Corpus

| File | Purpose |
|---|---|
| `valid-atlas.json` | large valid rocket from the examples |
| `valid-mission.json` | valid mission catalogue entry |
| `invalid-truncated.json` | truncated object |
| `invalid-deep.json` | nesting beyond the 64-level limit |
| `invalid-number.json` | non-finite number literal |
| `invalid-control.json` | raw control character inside a string |

The corpus is a seed set, not a claim of exhaustive coverage. Add any crash or
rejection case to `fuzz/corpus` and rerun.

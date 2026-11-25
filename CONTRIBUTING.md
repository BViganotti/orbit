# Contributing

Orbit is deliberately dependency-free and deterministic. Contributions should
preserve both properties.

## Before changing physics

- Add or update a test that fails for the old behaviour.
- State the frame, units and assumptions in `docs/physics.md`.
- Keep targeters deterministic; no wall-clock seeding, no random restarts.
- Never convert a failed targeting residual into a success.

## Before changing the API or UI

- Respect the parser and state limits in `docs/architecture.md`.
- Add a malformed-input case if you add a field.
- Keep the UI free of external URLs, fonts, images and scripts.
- Run the Playwright test after UI changes.

## Checks

```sh
cargo fmt --all
cargo build --release
cargo test
cargo test --release
cargo run --release -p orbit-tools --bin orbit-fuzz
cargo run --release -p orbit-tools --bin orbit-batch
python3 tests/playwright_studio.py
```

No CI workflows are accepted in this repository. Verification is local by
design.

## Commit style

Use a scoped prefix such as `feat(core):`, `fix(studio):`, `test(flight):`,
`docs:` or `build:`. Every commit should build and leave the tests green.

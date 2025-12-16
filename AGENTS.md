# Repository Guidelines

## Project Structure & Module Organization

- `src/`: root binary (`mdminecraft`) entrypoint and game loop.
- `crates/`: workspace crates grouped by subsystem (`core`, `world`, `render`, `net`, `server`, `client`, `ecs`, `physics`, `ui3d`, `assets`, `audio`, `scripting`, `cli`, `testkit`).
- `tests/`: integration tests and fixtures.
- `assets/` + `config/`: runtime data (textures/atlases; `blocks.json`, `recipes.json`, `controls.toml`, scripted inputs in `config/scripts/`).
- `tools/`: developer utilities (`atlas_packer`, `ecs_compare`).

## Build, Test, and Development Commands

```bash
cargo build            # fast dev build
cargo build --release  # optimized build
cargo run -- --auto-play
cargo run --bin mdminecraft-server --release
cargo run --bin mdminecraft-client --release
```

Quality checks (matches CI):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Coding Style & Naming Conventions

- Rust 2021 workspace; format with `rustfmt` (`rustfmt.toml` is committed).
- Prefer clippy-clean code; avoid `unsafe` unless clearly justified.
- Naming: `snake_case` for modules/functions, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts; keep crate names `mdminecraft-*`.
- Determinism is a core requirement—avoid time/OS-dependent behavior in simulation paths (see `docs/Deterministic-Coding-Handbook.md`).

## Testing Guidelines

- Use `cargo test --workspace` for quick coverage; add focused tests at the crate level when possible.
- Integration tests live in `tests/`; larger “worldtests” are typically named `*worldtest` and can be run directly, e.g.:
  `cargo test -p mdminecraft-world --test '*worldtest' -- --nocapture`.
- Some property/invariant tests may be `#[ignore]`: run with `cargo test -p mdminecraft-world -- --ignored`.

## Commit & Pull Request Guidelines

- Commit messages in this repo are short, imperative, and scoped by intent (examples from history: “Add …”, “Fix …”, “Update …”). Use a clear summary; add a body for rationale/risk when non-trivial.
- PRs should include: what changed, how to test (exact commands), and any determinism/networking considerations. Add screenshots/videos for UI/rendering changes.
- Keep PRs green: `fmt`, `clippy`, and `cargo test --workspace` should pass before review.

## Security & Configuration Tips

- Networking is TLS by default. For local dev against self-signed servers you can set `MDM_INSECURE_TLS=1` (do not use in production).
- Server certs can be provided via `MDM_SERVER_CERT_PATH` and `MDM_SERVER_KEY_PATH` (PEM, PKCS8).

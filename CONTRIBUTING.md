# Contributing to Crucible

## Prerequisites

- **Rust** — Install via [rustup](https://rustup.rs). The workspace uses the stable toolchain.
- **Stellar / Soroban** — The crucible crate depends on `soroban-sdk`. Ensure the `wasm32-unknown-unknown` target is installed: `rustup target add wasm32-unknown-unknown`
- **Backend** — The `backend` crate uses `sqlx` with PostgreSQL. Set `DATABASE_URL` in `backend/.env` before running migrations.
- **Node.js (optional)** — The frontend uses `npm` for dependency management and tests.

## Getting Started
git clone https://github.com/benelabs/crucible.git
cd crucible
cargo build --workspace
cargo test --workspace --all-features

text

## Quality Gates

Run these locally before pushing. CI runs them on every PR:
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo test --workspace --all-features
cargo build --package crucible --target wasm32-unknown-unknown

text

## Workspace Structure

| Crate | Description |
|-------|-------------|
| crucible | Core Soroban test framework with cost tracking, fixtures, and snapshots |
| crucible-macros | Derive macros for fixture attribute |
| backend | Axum-based API server with PostgreSQL |
| frontend | Vite + React dashboard |
| examples | Reference Soroban contracts using the framework |

## Common Commands
cargo test -p crucible --all-features
cargo test -p crucible-macros
cargo test -p backend
cd frontend && npm test
cargo doc --workspace --no-deps --all-features --open

text

## Pull Request Process

1. Fork and create a feature branch
2. Make changes, add tests
3. Run quality gates locally
4. Update docs if needed
5. Open PR referencing the issue

## Need Help?

Open an issue on the repository.

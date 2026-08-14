# Developer Guide

This guide explains the contributor workflow for the generated mpsc-log project.

## Spelling policy

Run `make spelling` to enforce en-GB-oxendict prose spelling. The generated
`typos.toml` starts from the shared estate dictionary, refreshes its untracked
local cache only when the authority is newer, and then applies the narrow
repository policy in `typos.local.toml`. Edit the local policy and regenerate
the configuration rather than changing generated entries by hand.

The pure dictionary schema, merge, and rendering logic lives in
`scripts/typos_rollout_dictionary.py`; reuse it only through the rollout and
generator entrypoints. `scripts/typos_rollout_cache.py` owns guarded HTTPS and
atomic persistence boundaries. Only connectivity failures may reuse stale or
tracked policy: HTTP status, validation, and local persistence errors fail
closed.

## Local Workflow

Use `make all` as the public entrypoint for formatting, linting, and tests.
`make lint` runs rustdoc, Clippy, and Whitaker. `make test` prefers
`cargo nextest run` and falls back to `cargo test` when cargo-nextest is not
available. `make audit` derives the Rust workspace root with `cargo metadata`,
logs workspace member manifests, and runs `cargo audit` once from the workspace
root. `make coverage` uses `cargo llvm-cov` with `lld`.

GitHub Actions Act validation lives in `.github/workflows/act-validation.yml`.
The main `.github/workflows/ci.yml` workflow deliberately does not run
`make test WITH_ACT=1`; the separate Act workflow runs those slower
container-backed checks in parallel.

## Tooling

Development builds use the standard LLVM backend for debug code generation.
On Linux targets, `.cargo/config.toml` configures clang to link with `mold`
so debug builds link quickly. An opt-in accelerated path, `make dev-build`
and `make dev-test`, applies the Cranelift codegen backend via
`tools/dev-fast/config.toml`; it requires a nightly toolchain and is never
applied to release, coverage, or verification builds. `rust-toolchain.toml`
retains the `llvm-tools-preview` and `rustc-codegen-cranelift-preview`
components so this opt-in path and `make coverage` work without an extra
`rustup component add`; `tools/dev-fast/config.toml` is what actually
switches the codegen backend on for a given invocation, not the toolchain
pin itself. Coverage generation uses `lld` because LLVM coverage tooling
expects LLVM-compatible linker behaviour.

Install `clang`, `lld`, `mold`, `python3`, and `cargo-audit` before running the
full generated workflow locally on Linux.

### Security audit ignores

Security audit jobs may set `CARGO_AUDIT_IGNORES` for narrowly scoped RustSec
advisories that affect unused or tooling-only dependency paths. Keep each
ignore tied to a documented runtime impact analysis, and remove it when the
affected dependency leaves the graph or the project starts using the advised
runtime path.

## Lint baseline

`Cargo.toml` carries the estate's phase 2 lint baseline directly under
`[lints.clippy]`, `[lints.rust]`, and `[lints.rustdoc]`; mpsc-log is a single
crate with no `[workspace]` table, so there is no separate
`[workspace.lints]` table to inherit from. `Cargo.toml` is the authoritative
list — consult it rather than this guide when checking whether a specific
lint is enabled or at what level.

Treat every denied lint as a real constraint. Where a violation is a genuine,
scheduled deferral rather than something to fix immediately, annotate the
site with `#[expect(clippy::<lint>, reason = "...")]`, never `allow`: an
`expect` that goes unfulfilled once the site is fixed becomes a compiler
warning, so the deferral backlog removes itself instead of rotting silently.

`clippy.toml` sets the complexity and nesting thresholds and the
`disallowed-methods` list that forbids calling `std::env::var`,
`std::env::var_os`, `std::env::set_var`, and similar functions directly;
inject an environment reader instead, so tests can stub it. The pinned
nightly toolchain (`rust-toolchain.toml`) supplies the `rustfmt`, `clippy`,
and `rust-analyzer` components the baseline and this workflow depend on.

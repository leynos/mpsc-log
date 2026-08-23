# Developer Guide

This guide explains the contributor workflow for the generated mpsc-log project.

## mpsc-log implementation architecture

`mpsc-log` is planned as a small domain core surrounded by adapters. The domain
owns the record model and the write, rotation, and retention policy. Adapters
own every external format and effect. The authoritative specification is the
[design document](mpsc-log-design.md); this section summarizes the boundary it
defines so contributors can place new code correctly.

`src/main.rs` owns process startup and `sysexits` mapping only. It constructs
the adapters, runs the domain, and translates the semantic error type into an
exit code. It carries no parsing, merging, rotation, or filesystem logic.

### Planned modules

| Module    | Layer       | Responsibility                                                                                                                        |
| --------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `args`    | Adapter     | Read process arguments and hand on the journal path and the raw selected `jo` field tail in order.                                    |
| `fields`  | Domain      | Interpret field words, object paths, coercion flags, and file-value forms into domain `Value` results.                                |
| `config`  | Adapter     | Parse and validate the sidecar TOML at the input boundary, converting `[defaults]` and `[schema]` data into domain types.             |
| `record`  | Domain      | Build the `Record`: merge defaults and CLI fields, apply coercion policy and object-path updates, and insert the generated timestamp. |
| `journal` | Domain      | Plan tail repair, rotation, retention, and append, and drive them through the `JournalStore` port.                                    |
| `errors`  | Domain      | Define the semantic error type. Exit-code mapping belongs to `src/main.rs`, not here.                                                 |
| `clock`   | Domain port | Declare the `Clock` port; an infrastructure implementation supplies the real instant.                                                 |
| `fs`      | Adapter     | Implement `JournalStore` over the real filesystem, alongside the fault-injection test double.                                         |

### The adapter boundary

The domain never names an external API. It works only with the domain `Record`
and `Value` types and with ports. Everything that reaches outside the process,
or that encodes an external format, is an adapter concern:

- CLI argument reads.
- TOML parsing of the sidecar.
- JSON serialization of the record.
- Filesystem operations.
- Locking.
- Compression.
- Process exit mapping.

The domain reaches time and persistence-related effects through two ports.
`Clock` supplies the invocation instant used for the generated `timestamp`
field. `JournalStore` performs every journal-directory effect: creating parent
directories, reading the sidecar, acquiring and releasing the journal lock,
inspecting and repairing the active tail, appending, truncating, renaming,
writing compression output, and syncing metadata.

Because both are traits, tests substitute a fixed clock and a fault-injecting
store without mutating global process state. See
[reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
for the injection patterns this repository expects.

When adding code, put format and effect handling in an adapter and keep policy
in the domain. If a domain module needs a new external effect, add a port
rather than importing the concrete API.

### Further reading

- [Design](mpsc-log-design.md) specifies the CLI contract, record model,
  ports, adapters, write protocol, and rotation protocol.
- [ADR 001: Lock file naming](adr-001-lock-file-naming.md) records how the
  journal lock path is derived and which suffixes are reserved.
- [ADR 002: Testing strategy](adr-002-testing-strategy.md) records how the
  required testing prongs apply to this design.
- [ADR 003: `jo` field syntax and duplicate keys](adr-003-jo-field-syntax-and-duplicate-keys.md)
  records the selected field syntax and last-wins duplicate-path semantics.

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

Development builds use the standard LLVM backend for debug code generation. On
Linux targets, `.cargo/config.toml` configures clang to link with `mold` so
debug builds link quickly. An opt-in accelerated path, `make dev-build` and
`make dev-test`, applies the Cranelift codegen backend via
`tools/dev-fast/config.toml`; it requires a nightly toolchain and is never
applied to release, coverage, or verification builds. `rust-toolchain.toml`
retains the `llvm-tools-preview` and `rustc-codegen-cranelift-preview`
components so this opt-in path and `make coverage` work without an extra
`rustup component add`; `tools/dev-fast/config.toml` is what actually switches
the codegen backend on for a given invocation, not the toolchain pin itself.
Coverage generation uses `lld` because LLVM coverage tooling expects
LLVM-compatible linker behaviour.

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
crate with no `[workspace]` table, so there is no separate `[workspace.lints]`
table to inherit from. `Cargo.toml` is the authoritative list — consult it
rather than this guide when checking whether a specific lint is enabled or at
what level.

Treat every denied lint as a real constraint. Where a violation is a genuine,
scheduled deferral rather than something to fix immediately, annotate the site
with `#[expect(clippy::<lint>, reason = "...")]`, never `allow`: an `expect`
that goes unfulfilled once the site is fixed becomes a compiler warning, so the
deferral backlog removes itself instead of rotting silently.

`clippy.toml` sets the complexity and nesting thresholds and the
`disallowed-methods` list that forbids calling `std::env::var`,
`std::env::var_os`, `std::env::set_var`, and similar functions directly; inject
an environment reader instead, so tests can stub it. The pinned nightly
toolchain (`rust-toolchain.toml`) supplies the `rustfmt`, `clippy`, and
`rust-analyzer` components the baseline and this workflow depend on.

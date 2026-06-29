# Documentation contents

[Documentation contents](contents.md) is the index for mpsc-log's documentation
set.

## Project guides

- [Context](context.md) defines the project vocabulary for journals,
  sidecars, rotation, and df12-build telemetry.
- [Design](mpsc-log-design.md) specifies the CLI, file formats, locking,
  rotation, failure handling, and verification strategy for `mpsc-log`.
- [Terms of reference](terms-of-reference.md) defines the product problem,
  users, scope boundaries, constraints, and open questions for `mpsc-log`.
- [Roadmap](roadmap.md) translates the design and terms of reference into
  review-sized implementation tasks.
- [User guide](users-guide.md) explains how to use the generated project and
  its public build and test commands.
- [Developer guide](developers-guide.md) explains the local workflow and
  implementation tooling for contributors.
- [Repository layout](repository-layout.md) explains the generated project's
  top-level files, directories, and ownership boundaries.
- [Documentation style guide](documentation-style-guide.md) defines the
  spelling, structure, Markdown, Architecture Decision Record (ADR), Request
  for Comments (RFC), and roadmap conventions used by this documentation set.

## Rust reference material

- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  explains how to keep tests deterministic by injecting environment, clock,
  filesystem, and other external dependencies.
- [Rust doctest Don't Repeat Yourself guide](rust-doctest-dry-guide.md)
  explains how to write maintainable, executable Rust documentation examples.
- [Rust testing with `rstest` fixtures](rust-testing-with-rstest-fixtures.md)
  explains fixture-based, parameterized, and asynchronous testing with `rstest`.

## Engineering practice

- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md)
  explains cognitive complexity, the bumpy-road antipattern, and refactoring
  approaches for maintainable code.
- [Scripting standards](scripting-standards.md) explains the preferred Python
  scripting stack, command execution patterns, and test expectations for helper
  scripts.


## Design artefacts

- [ADR 001: Lock file naming](adr-001-lock-file-naming.md) records how the
  journal lock path is derived and which coordination/configuration suffixes
  are reserved.
- [ADR 002: Testing strategy](adr-002-testing-strategy.md) records how the
  repository's required testing prongs apply to the `mpsc-log` design.
- [mpsc-log event schema](mpsc-log-event-schema.json) defines the initial JSON
  Schema for journal records.
- [mpsc-log sidecar example](mpsc-log-sidecar.example.toml) shows the TOML
  configuration shape used by the design.

# Architectural decision record (ADR) 002: Testing strategy

## Status

Accepted on 2026-06-29. The project will apply the testing prongs mandated in
`AGENTS.md` as a layered strategy tied to the `mpsc-log` design's contracts,
not as a separate testing phase.

## Date

2026-06-29.

## Context and problem statement

`mpsc-log` is a short-lived Rust CLI that appends one JSON object to a shared
JSON Lines journal while handling sidecar configuration, type coercion,
locking, repair, rotation, compression, and bounded lock waiting. The design's
highest-risk behaviours are externally observable: concurrent writers must not
lose records, rotation must preserve every successful entry, partial tails must
be repaired, and diagnostics must let non-interactive agents distinguish usage,
data, configuration, I/O, and timeout failures.

`AGENTS.md` mandates several testing prongs for this repository: unit tests,
behavioural tests, snapshots, end-to-end tests, property tests, bounded model
checking, and Verus proofs where contractual business logic warrants exhaustive
proof. The project needs one decision record explaining how those prongs apply
to the proposed design so implementers do not either under-test concurrency and
persistence or create standalone testing theatre that does not improve
confidence.

## Decision drivers

- The CLI's main promise is durable append under contention, so process-level
  and filesystem-level evidence matter more than line coverage alone.
- The design already separates pure domain logic from clock and filesystem
  adapters, which should make deterministic and fault-injection tests practical.
- `AGENTS.md` requires `rstest`, `rstest-bdd` where applicable, `insta` where
  output variants matter, end-to-end tests for observable workflows, and
  property-based or formal methods for invariants over ranges of states.
- Tests must remain review-sized with development tasks. Unit and behavioural
  tests belong with their implementation work; only cross-feature end-to-end,
  combinatorial, or formal hardening suites should stand alone.
- Direct environment mutation in tests is forbidden unless guarded through
  shared helpers as a last resort. The current design should avoid relying on
  mutable process environment state.

## Requirements

### Functional requirements

- Verify that each successful invocation appends exactly one complete JSON
  object line.
- Verify that concurrent successful invocations produce the same number of
  complete records as successful process exits.
- Verify that size-only rotation, scheduled rotation, gzip compression, and
  retention preserve successful records.
- Verify that malformed tails and injected write failures do not leave partial
  final records after the next invocation.
- Verify the selected `jo` field syntax, object-root enforcement, sidecar
  precedence, schema-guided coercion, explicit coercion flags, default
  timestamp insertion, and stable exit-code mapping.
- Verify df12-build fixture records against the JSON Schema and sidecar
  coercion contract.

### Technical requirements

- Use `rstest` fixtures and parameterized cases for unit and integration tests.
- Use `rstest-bdd` for externally observable CLI scenarios where the
  Given/When/Then shape clarifies user-facing behaviour.
- Use `insta` snapshots only for stable, reviewer-useful output boundaries,
  pairing snapshots with semantic assertions and normalizing nondeterministic
  fields.
- Use process-level end-to-end tests for CLI workflows, concurrent appends,
  rotation, and df12-build fixture invocations.
- Use `proptest` for generated field words, object-path merges, sidecar/CLI
  precedence, timestamp override cases, tail-repair inputs, and rotation-plan
  invariants.
- Use `kani` for bounded state-machine checks when a pure planner has a compact
  state space, such as generation shifting, scheduled period grouping, or
  append/repair transitions.
- Use Verus only for introduced lemmas or contractual business logic whose
  safety property is important enough to justify proof maintenance.
- Keep filesystem, time, and failure injection behind explicit adapters rather
  than mutating global state.

## Options considered

### Option A: Unit and CLI smoke tests only

This option would test parsers, record construction, and a few happy-path CLI
commands. It is fast and easy to maintain, but it does not prove the
concurrency, repair, or rotation promises that justify the tool.

### Option B: Apply every testing tool to every feature

This option would require unit, behavioural, snapshot, end-to-end,
property-based, model-checking, and proof coverage for every change. It is
maximally strict, but it would slow review, produce weak tests for features
that do not benefit from a given prong, and make formal tools feel ceremonial.

### Option C: Risk-based layered strategy

This option maps each testing prong to the design surface it can validate best.
Pure logic gets unit and property tests. User-facing workflows get behavioural
and end-to-end tests. Stable text or file-shape contracts get narrow snapshots.
Concurrency, rotation, and repair get stress, fault-injection, and model-based
coverage. Formal proof is reserved for compact contractual invariants.

| Topic                      | Option A      | Option B                | Option C                      |
| -------------------------- | ------------- | ----------------------- | ----------------------------- |
| Concurrency confidence     | Weak          | Strong but costly       | Strong where required         |
| Review size                | Small         | Often too large         | Review-sized by risk          |
| Formal methods             | Absent        | Over-applied            | Applied to compact invariants |
| Alignment with `AGENTS.md` | Incomplete    | Literal but inefficient | Complete and targeted         |
| Maintenance cost           | Low initially | High                    | Proportional to risk          |

_Table 1: Comparison of testing strategy options._

## Decision outcome / proposed direction

Adopt Option C, the risk-based layered testing strategy.

Every implementation task must include the unit, behavioural, property, or
snapshot coverage needed for the behaviour it introduces. Dedicated roadmap
tasks remain appropriate for cross-feature evidence that exceeds one pull
request, such as multi-process stress testing, pairwise CLI/configuration
coverage, concurrent rotation end-to-end tests, and broader formal hardening of
shared planners.

The testing prongs apply as follows:

- Unit tests with `rstest` cover pure domain logic in `args`, `fields`,
  `config`, `record`, `errors`, `clock`, and rotation-planning helpers.
- Behavioural tests with `rstest-bdd` cover user-visible CLI behaviours:
  successful append, unsupported syntax, sidecar precedence, diagnostics,
  timeout, repair, and rotation scenarios.
- Snapshot tests with `insta` cover stable outputs that reviewers benefit from
  seeing as artefacts: help text, one-line diagnostics, canonical fixture
  records after nondeterministic normalization, and rotation filename matrices.
- End-to-end tests cover real binary invocations against temporary directories,
  including parent-directory creation, simultaneous first writes, concurrent
  appends, forced size rotation, scheduled rotation, compression, and
  df12-build fixture commands.
- Property tests with `proptest` cover input and state ranges for field
  parsing, object-path insertion, type coercion, merge precedence, partial-tail
  repair, and rotation-plan invariants.
- Bounded model checks with `kani` cover compact pure planners where exhaustive
  small-state exploration is more useful than randomized testing, especially
  generation shifting and scheduled-period retention.
- Verus proofs are reserved for explicit lemmas or contractual business logic
  introduced during implementation. They are not required for ordinary glue
  code, parser plumbing, or tests that can be more clearly expressed with
  `rstest`, `proptest`, or `kani`.

## Goals and non-goals

- Goals:
  - Tie each required testing prong to a concrete `mpsc-log` design surface.
  - Keep tests deterministic through clock and filesystem injection.
  - Make concurrency, rotation, repair, and failure-mode evidence release
    blockers.
  - Keep implementation tasks review-sized by embedding ordinary tests in the
    task that introduces the behaviour.
- Non-goals:
  - Mandate every testing tool for every implementation task.
  - Treat coverage percentage as a substitute for contract evidence.
  - Prove network-filesystem correctness before the design accepts that
    platform promise.
  - Add `max_age` retention tests while `max_age` remains out of scope.

## Migration plan

1. Add test support before the first feature slice lands.
   - Create shared `rstest` fixtures for temporary directories, fixed clocks,
     filesystem adapters, command invocation, and decoded JSONL records.
   - Add helper modules under `tests/` or `src/` behind `#[cfg(test)]` where
     they preserve ownership boundaries.
2. Add domain tests with each implementation module.
   - Pair parser, sidecar, record, error, and rotation-planner work with
     `rstest` cases and `proptest` strategies for their accepted input spaces.
3. Add behavioural and end-to-end suites once the CLI can append a record.
   - Use `rstest-bdd` for user-facing scenarios and process-level tests for
     real binary execution.
4. Add concurrency, repair, and rotation hardening as the filesystem adapter
   lands.
   - Exercise multi-process contention, lock timeout, partial-tail repair,
     injected I/O failures, size rotation, scheduled rotation, gzip, and
     retention.
5. Add model checking or proofs only when the implementation introduces a pure
   invariant-bearing planner or lemma.
   - Prefer `kani` for bounded state machines and Verus for explicit lemmas
     whose proof is clearer than a large generated test matrix.

## Known risks and limitations

- Multi-process stress tests can be timing-sensitive. They must assert durable
  outcomes, not exact scheduling order.
- Snapshot tests can become brittle if they capture broad objects or raw
  timestamps. They must stay narrow and normalize nondeterministic fields.
- Formal tools add maintenance cost. They should protect compact invariants,
  not duplicate straightforward example tests.
- The initial correctness claim remains limited to local filesystems until a
  separate platform verification matrix names and validates other filesystem
  behaviours.

## Architectural rationale

The chosen strategy follows the design's architecture. Pure record-building and
rotation-planning code can be tested exhaustively and deterministically.
Filesystem effects pass through adapters, allowing fault injection without
global state mutation. User-facing CLI behaviour is verified at the process
boundary because agents experience the tool through exit codes, diagnostics,
and files on disk. The result keeps the repository's mandated testing prongs
connected to real product risk instead of distributing them mechanically across
the codebase.

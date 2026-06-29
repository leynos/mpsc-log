# Architectural decision record (ADR) 001: Lock file naming

## Status

Accepted on 2026-06-29. `mpsc-log` reserves coordination filenames and derives
one journal lock by appending `.lock` to the complete journal filename in the
same directory.

## Date

2026-06-29.

## Context and problem statement

`mpsc-log` serializes repair, sidecar reads, rotation, compression, and append
through one advisory lock per journal. Every safety claim in the design depends
on all cooperating invocations choosing the same lock for the same journal and
choosing different locks for different journals.

The obvious rule, `<journal filename>.lock` in the same directory, is simple
but needs collision boundaries. Names such as `run`, `run.jsonl`,
`run.jsonl.lock`, and sidecar paths derived by extension replacement can
otherwise create hidden ambiguity:

- `run` should not silently collide with `run.jsonl`;
- `run.jsonl.lock` is naturally the lock path for `run.jsonl`, but it could
  also be supplied as a journal path unless reserved;
- `run.toml` can be both a journal path and the extension-replaced sidecar path
  for `run` unless `.toml` journal paths are rejected;
- journals that share a stem, such as `run` and `run.jsonl`, share the sidecar
  `run.toml` under the design's extension-replacement rule.

The project needs this decision before implementing filesystem safety, because
lock naming, sidecar derivation, and reserved suffixes define what the writer
can safely create under contention.

## Decision drivers

- Lock naming must be deterministic from the caller-provided journal path.
- The lock path must not use extension replacement; otherwise `run` and
  `run.jsonl` can become ambiguous.
- Coordination artefacts must be reserved so callers cannot accidentally append
  JSONL records into another journal's lock file.
- The sidecar derivation rule is already part of the design and should remain
  easy to explain.
- The rule must work before the journal exists and while several first writers
  create parent directories concurrently.

## Requirements

### Functional requirements

- Derive one lock path for each accepted journal path.
- Ensure accepted journal paths cannot name another journal's lock file.
- Ensure accepted journal paths cannot equal their own sidecar path.
- Keep `run` and `run.jsonl` lock paths distinct.
- Keep the sidecar extension-replacement rule explicit when multiple journal
  names share one stem.

### Technical requirements

- Create parent directories before opening the lock file.
- Open the lock file with create semantics and acquire the exclusive lock before
  reading sidecar configuration, repairing tails, rotating, compressing, or
  appending.
- Treat `.lock` journal paths as invalid because `.lock` is reserved for
  coordination artefacts.
- Treat `.toml` journal paths as invalid when the derived sidecar path would be
  the journal path itself.
- Keep lock naming local to the journal directory; do not introduce a global
  lock directory or daemon.

## Options considered

### Option A: Append `.lock` to the full journal filename and reserve suffixes

For an accepted journal path, derive the lock path by appending `.lock` to the
complete filename in the same directory. Reject journal filenames ending in
`.lock`, and reject journal filenames whose derived sidecar path equals the
journal path.

Examples:

| Journal path     | Lock path         | Sidecar path | Accepted                        |
| ---------------- | ----------------- | ------------ | ------------------------------- |
| `run`            | `run.lock`        | `run.toml`   | Yes                             |
| `run.jsonl`      | `run.jsonl.lock`  | `run.toml`   | Yes                             |
| `run.ndjson`     | `run.ndjson.lock` | `run.toml`   | Yes                             |
| `run.jsonl.lock` | None              | None         | No, `.lock` is reserved         |
| `run.toml`       | None              | None         | No, sidecar would equal journal |

_Table 1: Filename outcomes for the accepted naming policy._

This option keeps lock derivation simple, avoids lock collisions between
extension variants, and makes reserved coordination/configuration names
explicit.

### Option B: Replace the journal extension with `.lock`

This option would make `run.jsonl` use `run.lock`. It is short, but it collides
with the lock path for `run` and repeats the ambiguity already present in
extension-replaced sidecar names.

### Option C: Use a hidden lock filename

This option would use a name such as `.run.jsonl.lock` in the same directory.
It avoids some visible filename clutter, but it does not remove the need for
reserved names, makes operator inspection harder, and adds platform-specific
hidden-file expectations without improving the safety claim.

### Option D: Use a central lock directory

This option would hash canonical journal paths into a shared lock directory. It
can avoid adjacent artefacts, but it introduces global state, path canonicalize
questions, lifecycle cleanup, and permission behaviour that are
disproportionate for a local CLI.

| Topic                              | Option A | Option B | Option C | Option D |
| ---------------------------------- | -------- | -------- | -------- | -------- |
| Same-stem collision risk           | Low      | High     | Low      | Low      |
| Operator inspectability            | High     | High     | Lower    | Lower    |
| Requires global state              | No       | No       | No       | Yes      |
| Handles `run.jsonl.lock` ambiguity | Yes      | No       | Partly   | Partly   |
| Complexity                         | Low      | Low      | Medium   | High     |

_Table 2: Comparison of lock naming options._

## Decision outcome / proposed direction

Adopt Option A.

The writer derives lock paths by appending `.lock` to the complete accepted
journal filename in the same directory:

```plaintext
<journal-dir>/<journal-file-name>.lock
```

The operation appends to the complete filename; it does not replace an
extension. The lock for `run` is `run.lock`. The lock for `run.jsonl` is
`run.jsonl.lock`.

The suffix `.lock` is reserved for lock files. A caller-supplied journal path
whose final filename ends in `.lock` fails before append because it can be
mistaken for another journal's coordination artefact. A caller-supplied journal
path whose derived sidecar path equals the journal path also fails;
practically, that reserves `.toml` journal filenames under the current sidecar
rule.

Sidecar configuration still uses extension replacement: `run.jsonl` and
`run.ndjson` both use `run.toml`, while `run` also uses `run.toml`. That shared
sidecar is intentional under the current design. Callers needing independent
configuration must choose distinct stems or directories, such as
`run-jsonl.jsonl` and `run-raw.ndjson`.

## Goals and non-goals

- Goals:
  - Make lock naming deterministic and collision-resistant for accepted journal
    paths.
  - Reserve coordination and configuration artefact names before implementation.
  - Preserve the simple adjacent lock-file model.
  - Keep sidecar sharing by stem explicit rather than accidental.
- Non-goals:
  - Provide distributed locking across hosts.
  - Canonicalize paths across symlinks or mount aliases beyond normal
    filesystem API behaviour.
  - Replace sidecar extension derivation with a different configuration scheme.
  - Permit arbitrary journal filenames when they collide with coordination
    artefacts.

## Migration plan

1. Add journal-path validation before opening the lock file.
   - Reject final filenames ending in `.lock`.
   - Reject paths whose derived sidecar path equals the journal path.
2. Implement lock-path derivation as a pure function and test the examples in
   Table 1.
3. Use the derived lock path for the complete critical section: sidecar read,
   tail repair, rotation, compression, and append.
4. Document reserved suffixes and sidecar sharing in the users' guide when the
   CLI implementation lands.

## Known risks and limitations

- The rule does not prevent two accepted journal paths with the same stem from
  sharing a sidecar. That is documented as intentional configuration sharing.
- The rule does not solve symlink aliasing or path canonicalization across
  mount points. Those remain outside the local-filesystem correctness claim.
- Operators may still choose confusing stems, such as `run.locked.jsonl`; those
  are accepted because they do not use the reserved final `.lock` suffix.

## Architectural rationale

The accepted rule keeps the lock path adjacent to the data it protects and
removes hidden ambiguity around lock artefacts. Appending `.lock` to the full
filename keeps `run` and `run.jsonl` separate, while reserving `.lock` journal
paths prevents a caller from treating another journal's lock as data. The
decision is deliberately conservative because every later concurrency, repair,
rotation, and timeout guarantee assumes all cooperating invocations agree on
the same coordination boundary.

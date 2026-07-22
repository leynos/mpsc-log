# Architectural decision record (ADR) 003: `jo` field syntax and duplicate keys

## Status

Accepted on 2026-06-29. `mpsc-log` uses selected `jo`-inspired field syntax for
object records, but does not promise textual JSON compatibility with `jo`.
Duplicate writes to the same object path resolve with last-wins semantics.

## Date

2026-06-29.

## Context and problem statement

`mpsc-log` should feel familiar to workflow authors who already use `jo` to
build JSON from shell arguments. The product goal is not to be a drop-in `jo`
replacement. It is a reliable multi-process JSON Lines writer whose root record
is always an object and whose output is produced through `serde_json::Map`.

That distinction matters for duplicate object keys. Textual JSON can contain
repeated object names, and `jo` can produce those names in its output. A JSON
object map cannot preserve repeated textual keys. Once `mpsc-log` chooses a map
as its internal representation, duplicate writes must either be rejected or
collapsed into one value.

The project needs an explicit compatibility statement before implementing the
argument parser, merge rules, sidecar schema interaction, and tests.

## Decision drivers

- The CLI should preserve the ergonomic field words that make `jo` useful for
  shell callers.
- The root record must remain a JSON object so defaults, telemetry fields, and
  object-path writes have predictable names.
- Consumers of JSON Lines logs usually parse records into maps, so repeated
  textual keys would be fragile even if the writer could emit them.
- The implementation should use structured JSON serialization rather than
  manually assembling JSON text.
- Duplicate-path handling must match sidecar merge precedence and CLI
  left-to-right processing.

## Requirements

### Functional requirements

- Accept selected `jo`-inspired field words for object records.
- Support explicit coercion flags, schema-guided coercion, object paths, and
  file-value forms that preserve an object root.
- Reject `jo` features that would produce a non-object root or require textual
  output compatibility.
- Define duplicate writes to the same object path deterministically.
- Document that `mpsc-log` is `jo`-inspired, not fully `jo` compatible.

### Technical requirements

- Store records in `serde_json::Map` before serialization.
- Process CLI field words in argument order.
- Let later writes to the same object path replace earlier writes.
- Pair the duplicate-key behaviour with parameterized tests covering top-level
  keys, nested object paths, sidecar defaults, and explicit coercion flags.

## Options considered

### Option A: Selected `jo`-inspired syntax with last-wins duplicate paths

This option accepts the object-record syntax needed by `mpsc-log` and resolves
duplicate writes by replacing the earlier value at the same object path.

Examples:

| Arguments                                      | Result              |
| ---------------------------------------------- | ------------------- |
| `status=started status=done`                   | `{"status":"done"}` |
| `task.id=1 task.id=2` with `-d .`              | `{"task":{"id":2}}` |
| sidecar `status = "queued"`, CLI `status=done` | `{"status":"done"}` |

_Table 1: Last-wins examples for duplicate paths._

This option matches the existing merge model: sidecar defaults seed the record,
CLI fields are processed left to right, and each later CLI write wins at its
path.

### Option B: Reject duplicate paths

This option would fail when a CLI field writes a path already set by a sidecar
default or earlier CLI field. It is strict, but it makes deliberate overrides
awkward and complicates the existing precedence model.

### Option C: Preserve textual duplicate JSON object names

This option would attempt closer textual compatibility with `jo` by emitting
duplicate JSON object names. It conflicts with `serde_json::Map`, makes schema
coercion and object-path updates harder to reason about, and produces records
that many consumers collapse differently.

### Option D: Full `jo` compatibility

This option would expand the CLI toward all `jo` behaviours, including array
roots and formatting options. It conflicts with the object-root logging
contract and would pull the tool away from its append-safe journal purpose.

| Topic                      | Option A | Option B | Option C | Option D |
| -------------------------- | -------- | -------- | -------- | -------- |
| Object-root fit            | High     | High     | Medium   | Low      |
| Familiar shell syntax      | High     | Medium   | High     | High     |
| Textual `jo` compatibility | Partial  | Partial  | Higher   | High     |
| Structured JSON safety     | High     | High     | Low      | Medium   |
| Implementation complexity  | Low      | Medium   | High     | High     |

_Table 2: Comparison of field syntax and duplicate-key options._

## Decision outcome / proposed direction

Adopt Option A.

`mpsc-log` describes its CLI as `jo`-inspired or as selected `jo` field syntax.
It does not claim full `jo` compatibility. The `jo` manual remains prior art
for familiar field words and coercion expectations, but the `mpsc-log` contract
is the subset recorded in the design and this ADR.

Duplicate writes to the same object path use last-wins semantics:

1. Sidecar defaults seed the object.
2. CLI fields are processed in argument order.
3. Each CLI field replaces any existing value at its object path.
4. Explicit coercion flags affect the field they annotate before the value is
   written.
5. The generated timestamp is inserted only when no `timestamp` field exists
   after defaults and CLI fields.

This means last-wins applies to duplicate top-level keys, duplicate nested
paths, and CLI overrides of sidecar defaults. It does not preserve repeated
textual JSON names.

## Goals and non-goals

- Goals:
  - Make the compatibility boundary honest for users and implementers.
  - Preserve the familiar shell-friendly field forms needed for logging.
  - Keep object-root JSON serialization map-based and deterministic.
  - Align duplicate-path behaviour with merge precedence.
- Non-goals:
  - Preserve repeated textual JSON object names.
  - Implement `jo` formatting, pretty-printing, array-root, or version options.
  - Guarantee compatibility with every `jo` edge case.
  - Define organization-wide schema governance for duplicate telemetry fields.

## Migration plan

1. Update product documentation to use `jo`-inspired or selected `jo` field
   syntax rather than full compatibility language.
2. Implement parser support only for the accepted object-record forms listed in
   the design.
3. Add tests for last-wins duplicate paths across sidecar defaults, CLI fields,
   explicit coercion flags, and nested object paths.
4. Keep full `jo` compatibility in deferred scope unless a later ADR changes
   the object-root product boundary.

## Known risks and limitations

- Users expecting byte-for-byte `jo` output can be surprised by duplicate keys
  collapsing to one value.
- Last-wins can hide accidental repeated fields. The behaviour is predictable,
  but the implementation should keep diagnostics clear for invalid paths and
  unsupported forms.
- If future consumers require textual duplicate keys, the map-based record
  builder would need a larger redesign.

## Architectural rationale

The accepted rule matches the product's actual abstraction. `mpsc-log` is a
structured journal writer, not a JSON text generator. A map-backed record gives
sidecar defaults, schema coercion, explicit flags, object paths, and timestamp
defaulting one deterministic merge model. Last-wins duplicate handling is the
least surprising behaviour inside that model, provided the documentation does
not overstate compatibility with `jo`.

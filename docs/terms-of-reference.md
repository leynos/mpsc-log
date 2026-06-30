# mpsc-log – terms of reference

- **Status:** Draft v0.1.
- **Audience:** Product owner, engineering maintainers, agent-workflow
  authors, and future design reviewers.
- **Last substantive revision:** 2026-06-29.
- **Companion documents:**
  - [Documentation contents](contents.md).
  - [Users' guide](users-guide.md).
  - [Developer guide](developers-guide.md).
  - [Repository layout](repository-layout.md).
  - [Documentation style guide](documentation-style-guide.md).
  - [Context](context.md).
  - [Technical design](mpsc-log-design.md).
  - [Event schema](mpsc-log-event-schema.json).
  - [Sidecar example](mpsc-log-sidecar.example.toml).

## 1. Background and motivation

`mpsc-log` exists for multi-agent and scripted workflows that need several
independent processes to record structured events in one append-only log file.
The immediate problem is not creating JSON. Tools such as `jo` already turn
shell arguments into JSON objects.[^1] The gap is that agents also need a small
command-line interface (CLI) that appends each record to a shared JSON Lines
(JSONL) file without overwriting existing data, losing concurrent writes, or
corrupting the file during rotation.

The motivating context is local automation where several agents may run at the
same time and cannot coordinate through a long-lived service. Each agent should
be able to invoke the tool once, pass a target path and record fields, and then
continue. The tool must make the file-system coordination boring: safe file
creation, file locking, write atomicity, timeout handling, and rotation belong
inside the tool rather than being reimplemented by each caller.

The first concrete use case is a multi-agent Open Dynamic Workflows (ODW)
workflow for advancing `docs/roadmap.md` through planning, design review,
implementation, code review, expert review, integration, audit, and
remediation. Agents in that workflow need to append journal events to a file in
the workflow sidecar directory. The journal should capture enough real-run
telemetry to replace architectural guesses with measured behaviour.

This project is being defined before its design document. The existing
repository is a generated Rust application scaffold, so these terms of
reference treat the user brief and `Cargo.toml` package description as the
authoritative product inputs.

## 2. Domain

The product sits in structured local logging for automated command runners. Its
records are JSON objects serialized as one JSON value per line. JSONL keeps the
log easy to append, stream, grep, split, and ingest into later tools without
requiring the whole file to be rewritten.

The command accepts the log path as its first argument. Later arguments
describe fields using selected `jo`-inspired key and value words, including
type coercion flags and object paths. The CLI is not textually compatible with
all `jo` output: the root value must always be an object, and duplicate writes
to the same object path use last-wins semantics.

The default entry includes a `timestamp` field containing a Coordinated
Universal Time (UTC) timestamp captured when the command is invoked. The
timestamp uses RFC 3339, the Internet timestamp profile of ISO 8601.[^2]

The tool also reads a sidecar TOML file next to the log file. The sidecar has
the same base filename as the log file and a `.toml` extension. It defines
rotation configuration, type-coercion schema, and default field values.

The term "sidecar" has two relevant meanings in the current domain. The ODW
workflow has a workflow sidecar directory where run artefacts belong.
`mpsc-log` also has a sidecar TOML configuration file next to the selected
journal file. The design must keep those concepts distinct: callers choose the
journal path, and `mpsc-log` derives only its configuration path from that
journal path.

The first journaled telemetry set is operational evidence for the df12-build
workflow:

- phase timings and design-review or code-review round counts;
- task outcomes grouped by roadmap shape, including task size, work-item count,
  changed-file count, dependency depth, phase kind, review failures,
  implementation failures, merge conflicts, and audit follow-ups;
- CodeRabbit wait times, HTTP 429 frequency, deferred reviews, and retry
  success;
- audit yield by severity and remediation lane, including dropped findings,
  addenda, rerouted step tasks, and new roadmap steps;
- post-merge defect escape, including defects later found by dogfooding,
  continuous integration, users, or subsequent roadmap work.

## 3. Market context

The current alternatives each solve part of the problem:

- `jo` builds JSON from shell arguments but writes to standard output and does
  not provide locked append, timeout, file creation, or rotation semantics.
- Shell redirection with `>>` is convenient but leaves callers responsible for
  record construction, validation, rotation, and coordination under contention.
- `flock` can serialize shell commands, but each caller must compose locking,
  JSON construction, append mode, error handling, and rotation correctly.
- General logging frameworks target applications that own their logging stack;
  they are a poor fit for short-lived independent agents that need a single
  external command.
- System loggers and centralized observability products handle broader
  collection and retention concerns, but they introduce services, deployment,
  and integration work that is disproportionate for a local agent log.
- `logrotate` can rotate files, but it is scheduled external maintenance rather
  than a per-write safety mechanism integrated with concurrent appends.

The gap is a narrowly scoped CLI that combines selected `jo`-inspired record
construction with safe append and rotation behaviour for one local JSONL file.

## 4. Users and stakeholders

| Type                                       | Context                                                                                   | Cares about                                                                      | Will dislike or ignore                                                        | Current alternative                                               |
| ------------------------------------------ | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Primary user: agent workflow author        | Builds scripts, prompts, wrappers, or orchestration around multiple CLI-capable agents    | One command that reliably records structured events under concurrency            | Running a daemon, writing bespoke lock scripts, or debugging corrupted logs   | `jo` plus shell redirection, ad hoc scripts, or no structured log |
| Primary user: autonomous agent             | Invokes tools from a constrained shell environment during task execution                  | Simple argument contract, bounded waiting, machine-readable failure              | Interactive prompts, hidden global state, or non-deterministic output formats | Direct file append or caller-specific helper                      |
| Primary user: df12-build workflow operator | Runs ODW workflows that plan, review, implement, merge, audit, and remediate roadmap work | Evidence for tuning parallelism, review caps, audit loops, and remediation lanes | Manual reconstruction from transcripts, branch history, or final summaries    | Architectural guesses and scattered workflow output               |
| Secondary user: maintainer                 | Implements, reviews, and releases the Rust CLI                                            | Clear correctness requirements, testable failure modes, and stable docs          | Broad observability scope or vague compatibility promises                     | Generated project scaffold and maintainer convention              |
| Stakeholder: project sponsor               | Wants agents to leave durable audit trails without coordination overhead                  | Fewer lost events, easier review of concurrent runs, low setup cost              | Feature creep into a logging platform                                         | Manual run notes or scattered per-agent logs                      |
| Non-user: observability platform operator  | Runs centralized ingestion, querying, alerting, and retention infrastructure              | Fleet-level logging, dashboards, and policy controls                             | A local-file-only CLI                                                         | Existing observability stack                                      |

Table 1: Stakeholder mapping for the initial product boundary.

## 5. Job to be done

When an agent workflow launches several independent command-line processes, the
workflow author wants each process to append a structured event to the same
local log file, so they can inspect the run later without reconstructing events
from scattered output.

When a df12-build ODW run processes roadmap tasks through planning, review,
implementation, integration, and audit, the workflow operator wants agents to
append structured journal events into the workflow sidecar directory, so they
can tune workflow defaults from real-run telemetry rather than from design-time
intuition.

The functional dimension is durable structured append under contention. The
emotional dimension is confidence that a missing or malformed log entry points
to a real failure rather than a race in the logging helper. The social
dimension is that maintainers can review an agent run from one readable
artefact rather than trusting a caller's informal summary.

## 6. Scope

### 6.1 Goals

- Accept a log-file path as the first CLI argument and interpret following
  arguments as record fields.
- Support the selected `jo`-inspired key/value syntax needed for object
  records, including type coercion flags and object paths.
- Reject any invocation that would produce a non-object root.
- Add a default `timestamp` field using an invocation-time UTC timestamp unless
  configuration or CLI input explicitly overrides it according to the final
  precedence rules.
- Append exactly one valid JSON object followed by a newline for each successful
  invocation.
- Prevent concurrent calls from overwriting existing logs or colliding with
  each other while creating, appending to, or rotating the file.
- Create missing parent directories for the log-file path automatically when
  filesystem permissions allow it.
- Use locking and safe writes with a five-second default timeout.
- Gracefully handle simultaneous attempts to create the log file and sidecar
  coordination artefacts.
- Rotate by default after the active log reaches 1 MiB.
- Optionally rotate on hourly, daily, or weekly UTC time boundaries, with
  interim size splits if the active log reaches the size threshold before the
  next boundary.
- Compress rotated logs after the fourth rotation, subject to the final naming
  and retention policy.
- Read a sidecar TOML file for rotation configuration, schema-guided type
  coercion, and default field values.
- Surface failures through stable exit codes and diagnostics suitable for
  non-interactive agents.
- Support journal records for the df12-build ODW workflow's phase timings,
  review rounds, task-shape outcomes, CodeRabbit waits and 429s, audit yield,
  remediation lane outcomes, and post-merge defect escape.

### 6.2 Non-goals

- Centralized log ingestion, search, dashboards, alerting, and retention policy
  are out of scope; users needing those should ship the JSONL output into an
  observability system.
- A long-running daemon or background service is out of scope; the product is a
  short-lived CLI.
- Preserving chronological file order by timestamp is out of scope. The default
  timestamp is captured at invocation time, so entries can appear out of order
  when one caller waits behind another.
- Full `jo` feature parity and textual duplicate-key preservation are out of
  scope where they conflict with the object-root logging contract.
- Cross-host distributed locking is out of scope unless a later design
  explicitly accepts the complexity and platform constraints.
- Querying, filtering, formatting, or editing historical log entries is out of
  scope; readers can use existing JSONL and shell tooling.
- Log schema governance for an organization is out of scope. The sidecar schema
  exists to coerce one tool's input, not to become a central event taxonomy.
- Computing dashboards, recommendations, or automatic tuning for df12-build is
  out of scope. `mpsc-log` records the evidence; analysis can happen in later
  tooling.
- Discovering the ODW workflow sidecar directory is out of scope unless a later
  integration contract adds it. Callers pass the concrete journal path.

## 7. Success criteria

### 7.1 User-facing success

- A workflow author can replace an ad hoc `jo >> file.jsonl` call with
  `mpsc-log` without losing the ability to express nested object fields and
  basic type coercions.
- Stress tests with many concurrent invocations produce the same number of
  valid JSONL records as successful command exits.
- Simultaneous first writes to a missing log file create exactly one usable log
  and do not truncate, replace, or interleave records.
- A first write to a log path in a missing directory tree creates the required
  parent directories when permissions allow it.
- Rotation during concurrent writes leaves every successful entry in either the
  active file or a rotated file.
- A df12-build run can record one journal entry per meaningful phase, review
  round, implementation attempt, CodeRabbit attempt, integration result, audit
  finding, remediation triage decision, and post-merge defect report.

### 7.2 Operational success

- Lock acquisition obeys the configured timeout and defaults to five seconds.
- Rotation defaults are predictable: 1 MiB active-file threshold and compression
  after four rotations.
- A failed invocation leaves the previous log state readable and does not
  produce partial JSON records.
- Diagnostics are useful to agents and humans: invalid arguments, timeout,
  malformed sidecar configuration, final-line corruption, and file-system
  failures are distinguishable.
- Journal writes add low enough overhead that agents can record telemetry at
  phase and attempt boundaries without changing workflow scheduling decisions.

### 7.3 Strategic success

- The tool remains small enough for agents to call as a normal utility rather
  than as an integration project.
- The terms of reference, design document, and user guide give future
  contributors enough boundary information to reject logging-platform feature
  requests cleanly.
- df12-build maintainers can compare observed phase durations, review-round
  counts, CodeRabbit waits, audit yield, and defect escape against
  `MAX_PARALLEL`, `MAX_DESIGN_ROUNDS`, and `MAX_REVIEW_ROUNDS`.

## 8. Constraints and assumptions

### 8.1 Hard constraints

- The first CLI parameter is the log-file path.
- Later parameters are key/value words using the accepted `jo`-inspired
  syntax.
- The root record must be a JSON object.
- The default lock timeout is five seconds.
- The default rotation threshold is 1 MiB.
- The default scheduled rotation policy is `none`.
- Scheduled rotation modes are `hourly`, `daily`, and `weekly`; they use UTC
  period boundaries and do not imply a `max_age` retention setting.
- Rotated logs are compressed after four rotations.
- The sidecar configuration file is TOML and derives its path from the log file
  path by replacing the filename extension with `.toml`.
- The default record includes `timestamp` unless the final precedence rules say
  otherwise.
- Missing parent directories for the log-file path are created automatically
  when permissions allow it.
- The tool must tolerate concurrent writers and concurrent initial file
  creation.
- The workflow sidecar directory is caller-owned. `mpsc-log` receives the
  journal file path explicitly and must not assume a df12-build directory
  layout.

### 8.2 Assumptions

- The initial target is a local filesystem with locking semantics that can
  protect cooperating `mpsc-log` processes. If users rely on network
  filesystems with weaker locking, the tool may need documented limitations or
  a different coordination strategy.
- Callers can pass shell arguments without needing standard input as the primary
  data channel. If ARG_MAX or large payloads become common, file-value support
  and streaming input need sharper requirements.
- Agent callers can handle non-zero exit codes and diagnostic text. If a caller
  cannot observe failures, reliable logging cannot be guaranteed from the CLI
  alone.
- Directory creation can fail because of permissions, read-only filesystems, or
  invalid paths. Those failures must leave any existing log readable and return
  diagnostics that distinguish directory creation from record construction.
- Sidecar defaults are stable enough to read during each invocation. If callers
  edit sidecar configuration while writes are running, the design must specify
  whether configuration reads are locked with log writes.
- RFC 3339 UTC timestamps are acceptable for the default timestamp field. If a
  later integration requires a different timestamp profile, that format becomes
  an explicit compatibility requirement.
- The df12-build ODW workflow can call `mpsc-log` at phase and attempt
  boundaries. If agents cannot call the CLI from those points, the first
  integration will need a wrapper or workflow-level helper.
- A single workflow run can identify its events with stable run, task, phase,
  agent, and attempt identifiers. If those identifiers are not available, later
  analysis will be limited to aggregate timing and count data.

### 8.3 Dependencies

- The `jo` manual is prior art for selected argument syntax, type coercion
  flags, and object-path expectations; `mpsc-log` compatibility is limited by
  the object-root and last-wins decisions.[^1]
- RFC 3339 is the timestamp reference for default `timestamp` values.[^2]
- The future technical design must choose Rust crates or platform APIs for JSON
  serialization, TOML parsing, file locking, gzip compression, time handling,
  and atomic file operations.

## 9. Open questions

| Question                                             | Why it matters                                                                                                                                                         | Criteria for resolution                                                                                                        | Suggested path                           |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------- |
| Which exact subset of `jo` syntax is in scope?       | `jo` includes arrays, file-value operators, duplicate-key behaviour, object paths, and coercion flags that may conflict with an object-root log contract.              | The design records accepted, rejected, and modified syntax with examples.                                                      | Resolved by ADR 003.                     |
| What are the sidecar precedence rules?               | Defaults, schema coercion, CLI values, and `timestamp` overrides can conflict.                                                                                         | A precedence table defines every conflict outcome.                                                                             | Technical design.                        |
| How should duplicate keys resolve?                   | JSON permits duplicate object names textually, but consumers often collapse them.                                                                                      | The product chooses reject, last-wins, first-wins, or compatibility behaviour.                                                 | Resolved by ADR 003.                     |
| What does "gzipping after 4 rotations" mean exactly? | Retention, naming, and compression timing affect concurrency and user expectations.                                                                                    | The rotation policy specifies filenames, retention count, compression trigger, and whether recent rotations remain plain text. | Technical design.                        |
| What lock protects rotation and sidecar reads?       | Append-only locking may not be enough when a process rotates while others are opening or creating files.                                                               | The design names the lock artefact and the critical sections it protects.                                                      | Technical design and stress tests.       |
| What platforms are supported at v1?                  | File-locking and atomic rename semantics differ across Unix, Windows, and network filesystems.                                                                         | The project declares supported platforms and test coverage.                                                                    | ADR candidate.                           |
| What are the stable exit codes?                      | Agents need to distinguish retryable timeout from invalid input or corrupted configuration.                                                                            | The user guide lists exit codes and diagnostics.                                                                               | User-guide update during implementation. |
| Is a Rust library API part of the product?           | A public library API expands compatibility and documentation obligations.                                                                                              | The roadmap states whether the crate is CLI-only or also exposes supported library functions.                                  | Product decision.                        |
| What is the df12-build event taxonomy?               | The first use case needs comparable records for phase timings, task shape, review outcomes, CodeRabbit attempts, audit findings, remediation lanes, and defect escape. | The design defines stable event names, required fields, optional fields, and schema versions.                                  | Technical design with workflow fixture.  |
| How are workflow events correlated?                  | Without run, task, phase, agent, attempt, branch, and commit identifiers, telemetry cannot explain review loops, merge conflicts, or escaped defects.                  | The workflow integration names required correlation fields and their source.                                                   | Integration design.                      |

## 10. Handoff

### 10.1 Context additions

The [context](context.md) document should continue to define the shared
vocabulary for:

- Agent workflow.
- Audit yield.
- CodeRabbit attempt.
- Defect escape.
- df12-build.
- Open Dynamic Workflows (ODW).
- JSON Lines (JSONL).
- Journal.
- Record.
- Remediation lane.
- Review round.
- Sidecar configuration.
- Workflow sidecar directory.
- Rotation.
- Active log.
- Rotated log.
- Type coercion.
- Object path.
- Lock timeout.
- Scheduled break.

### 10.2 ADR candidates

- Locking and atomic-write strategy across supported platforms.
- Rotation naming, retention, and compression policy.
- CLI-only product boundary versus supported library API.

### 10.3 Downstream readiness

This document is complete enough to start a technical design. The design should
not begin implementation until the timestamp standard, selected `jo` field
syntax, sidecar precedence, rotation policy, and platform support questions are
resolved or explicitly deferred.

## Appendix A. References

[^1]: Jan-Piet Mens,
      [`jo` manual](https://github.com/jpmens/jo/blob/master/jo.md),
    accessed 2026-06-29.

[^2]: G. Klyne and C. Newman, [RFC 3339: Date and time on the Internet:
    timestamps](https://www.rfc-editor.org/rfc/rfc3339), July 2002, accessed
    2026-06-29.

# mpsc-log roadmap

This roadmap translates the terms of reference and technical design into an
outcome-oriented delivery sequence for `mpsc-log`. It does not promise dates.
Each phase carries one testable idea at the GIST level; each step answers a
sequencing question; each task is a review-sized execution unit with explicit
dependencies and source citations.

The primary source documents are [terms of reference](terms-of-reference.md),
[technical design](mpsc-log-design.md), [context](context.md),
[event schema](mpsc-log-event-schema.json), and
[sidecar example](mpsc-log-sidecar.example.toml). No RFCs or ADRs exist yet,
so the first phase records the decisions that would otherwise force rework.

## 1. Foundational contracts and build spine

Idea: if `mpsc-log` ratifies its v1 contracts, crate boundary, and validation
spine before feature work starts, later slices can converge on one small CLI
instead of repeatedly reopening syntax, locking, and retention decisions.

This phase resolves the decisions that affect every subsequent pull request:
the accepted `jo` subset, public API boundary, filesystem support policy,
rotation naming, and the repository shape needed to validate a CLI whose main
risks are concurrency and persistence.

### 1.1. Ratify the v1 decisions that would otherwise cause rework

This step answers what `mpsc-log` v1 will promise to callers and what it will
explicitly leave out. Its outcome informs crate layout, user documentation,
error handling, and future compatibility work. See
mpsc-log-design.md §§3, 5, 8, 12-13 and terms-of-reference.md §§6, 8-10.

- [ ] 1.1.1. Record the accepted `jo` subset and duplicate-key behaviour in
  an ADR.
  - See mpsc-log-design.md §§2, 5, 13 and terms-of-reference.md §§6, 9.
  - Success: the ADR names supported forms, rejected options, object-root
    enforcement, object-path handling, and last-wins duplicate-key semantics.
- [ ] 1.1.2. Record the CLI-only v1 product boundary in an ADR.
  - See mpsc-log-design.md §§3, 10, 12-13 and terms-of-reference.md §§6, 9.
  - Success: the ADR states that `src/main.rs` owns process exit mapping and
    that library exports are internal until a later roadmap item changes that.
- [ ] 1.1.3. Record the filesystem and locking support policy in an ADR.
  - See mpsc-log-design.md §§2, 4, 7, 11, 13 and
    terms-of-reference.md §§6-8.
  - Success: the ADR defines the local-filesystem correctness claim, lock-file
    naming, unsupported network-filesystem caveat, and timeout semantics.
- [ ] 1.1.4. Record the rotation naming, compression, and retention policy in
  an ADR.
  - See mpsc-log-design.md §§3, 6, 8, 11, 13 and
    terms-of-reference.md §§6, 8-9.
  - Success: the ADR covers `schedule = "none"`, scheduled UTC period names,
    size-split suffixes, gzip-after-four behaviour, and the absence of
    `max_age`.

### 1.2. Establish the implementation skeleton and quality gates

This step answers whether the generated Rust scaffold can support the module
boundaries, dependency choices, and validation strategy described by the
design. It informs all later slices because every feature will pass through the
same CLI, domain, filesystem, and test seams. See mpsc-log-design.md §§2, 4,
10-12 and docs/repository-layout.md.

- [ ] 1.2.1. Replace the scaffold library with the module skeleton described
  by the design.
  - Requires steps 1.1.1-1.1.3.
  - See mpsc-log-design.md §§4, 10, 12.
  - [ ] Add `args`, `fields`, `config`, `record`, `journal`, `errors`,
    `clock`, and `fs` modules with module-level documentation.
  - [ ] Keep `src/main.rs` limited to startup and exit-code mapping.
  - Success: the crate builds with empty but documented module boundaries and
    no stable public API beyond the binary's current needs.
- [ ] 1.2.2. Add the implementation dependencies with documented ownership.
  - Requires 1.2.1.
  - See mpsc-log-design.md §2 and docs/developers-guide.md.
  - [ ] Add the selected crates for CLI parsing, JSON, TOML, time, locking,
    gzip, atomic writes, and semantic errors using caret requirements.
  - [ ] Document each dependency's purpose in the appropriate design or
    developer guide location.
  - Success: `make check-fmt`, `make lint`, and `make test` pass after the
    dependency update.
- [ ] 1.2.3. Build deterministic test seams for time and filesystem effects.
  - Requires 1.2.1 and 1.2.2.
  - See mpsc-log-design.md §§7, 10-12 and
    docs/reliable-testing-in-rust-via-dependency-injection.md.
  - [ ] Provide injectable clock and filesystem adapter boundaries for record
    timestamps, fault injection, and deterministic rotation fixtures.
  - Success: failures such as partial writes, metadata errors, and fixed
    timestamps can be exercised without mutating global process state.

## 2. Day-one structured journal entries

Idea: if one `mpsc-log` invocation can parse `jo`-style fields, apply sidecar
defaults, emit an RFC 3339 UTC timestamp, and append one JSONL object, the tool
already replaces the unsafe `jo >> file.jsonl` baseline for simple workflows.

This phase delivers the narrowest useful end-to-end command. It intentionally
starts without the full concurrency and rotation surface so that parser,
configuration, record-shaping, and diagnostics can stabilize against real CLI
examples before the filesystem protocol grows more complex.

### 2.1. Prove the CLI field contract can build one object record

This step answers whether the accepted `jo` subset can be implemented without
breaking the object-root logging contract. It informs sidecar coercion, error
classification, and the later combinatorial test matrix. See
mpsc-log-design.md §§2, 5, 10-12 and terms-of-reference.md §§2, 6, 8.

- [ ] 2.1.1. Implement positional argument parsing for the journal path and
  raw field tail.
  - Requires steps 1.1-1.2.
  - See mpsc-log-design.md §§5, 10, 12.
  - Success: the parser preserves field words and coercion flags in order and
    rejects missing journal paths with `EX_USAGE`.
- [ ] 2.1.2. Implement field-word parsing for the accepted `jo` object forms.
  - Requires 2.1.1.
  - See mpsc-log-design.md §§2, 5 and terms-of-reference.md §§2, 6.
  - [ ] Cover `key=value`, `key@value`, object paths, array appends, and
    bracketed object insertion.
  - [ ] Reject unsupported `jo` options and any non-object root outcome.
  - Success: representative `jo` examples produce typed intermediate values
    or stable usage errors.
- [ ] 2.1.3. Implement explicit and inferred type coercion for CLI values.
  - Requires 2.1.2.
  - See mpsc-log-design.md §§5-6 and context.md.
  - Success: explicit `-s`, `-n`, and `-b` flags override inference, empty
    assignments become `null`, valid JSON values parse as JSON, and other
    values remain strings.
- [ ] 2.1.4. Implement file-value forms with clear data and I/O errors.
  - Requires 2.1.2 and 2.1.3.
  - See mpsc-log-design.md §§5, 10.
  - Success: `key:=path`, `key=@path`, `key=:path`, and `key=%path` produce
    the documented values or `EX_DATAERR`/`EX_IOERR` diagnostics.

### 2.2. Deliver sidecar-backed record construction

This step answers whether configuration, schema coercion, defaults, and the
generated timestamp can merge deterministically. Its outcome informs the
external JSONL contract and df12-build event examples. See
mpsc-log-design.md §§2, 6, 9, 11-12,
mpsc-log-sidecar.example.toml, and mpsc-log-event-schema.json.

- [ ] 2.2.1. Implement sidecar path derivation, TOML loading, and semantic
  validation.
  - Requires 2.1.1.
  - See mpsc-log-design.md §§2, 6, 10 and
    mpsc-log-sidecar.example.toml.
  - Success: missing sidecars use defaults, malformed TOML returns
    `EX_DATAERR`, and semantically invalid configuration returns `EX_CONFIG`.
- [ ] 2.2.2. Implement deterministic record merging and schema-guided
  coercion.
  - Requires 2.1.3 and 2.2.1.
  - See mpsc-log-design.md §§6, 9, 11 and
    mpsc-log-event-schema.json.
  - Success: sidecar defaults, CLI fields, explicit flags, schema entries,
    and duplicate paths resolve according to one table-driven contract.
- [ ] 2.2.3. Generate the default RFC 3339 UTC `timestamp` field.
  - Requires 1.2.3 and 2.2.2.
  - See mpsc-log-design.md §§2, 6 and terms-of-reference.md §§2, 6, 8.
  - Success: records receive an invocation-time UTC timestamp unless a prior
    merge step already produced `timestamp`.

### 2.3. Append the first useful JSONL record

This step answers whether the command can complete a non-concurrent append
workflow with stable user-visible behaviour. It informs the later lock and
repair protocol because this slice defines the line format and diagnostics.
See mpsc-log-design.md §§3, 7, 10-12 and terms-of-reference.md §§5-7.

- [ ] 2.3.1. Implement compact JSON object serialization and newline append.
  - Requires steps 2.1-2.2.
  - See mpsc-log-design.md §§3, 7, 11-12.
  - Success: each successful invocation appends exactly one compact JSON
    object followed by `\n`.
- [ ] 2.3.2. Map semantic errors to process exit codes and diagnostics.
  - Requires steps 2.1-2.2.
  - See mpsc-log-design.md §10 and terms-of-reference.md §§6-7.
  - Success: success writes nothing to stdout, failures write one stderr
    diagnostic, and invalid input, data errors, I/O errors, and configuration
    errors use the documented statuses.
- [ ] 2.3.3. Document the day-one CLI usage in the users' guide.
  - Requires 2.3.1 and 2.3.2.
  - See mpsc-log-design.md §§5-6, 9-10 and
    terms-of-reference.md §§4-7.
  - Success: a workflow author can replace a simple `jo >> file.jsonl`
    example with an equivalent `mpsc-log` invocation and understand failures.

## 3. Trustworthy shared logging under contention

Idea: if the same CLI can create missing directories, serialize concurrent
writers with bounded lock waiting, and repair partial tails, multi-agent
workflows can treat journal writes as boring infrastructure rather than a
source of telemetry loss.

This phase turns the single-writer command into the product promised by the
terms of reference. It focuses on the one risk that motivates the tool:
several independent agents calling the same executable at the same time.

### 3.1. Prove first-write and lock acquisition are safe

This step answers whether concurrent invocations can create the same journal
and coordination artefacts without truncating or colliding. It informs the
critical-section protocol and timeout behaviour. See mpsc-log-design.md §§4,
7, 10-11 and terms-of-reference.md §§6-8.

- [ ] 3.1.1. Create missing parent directories and coordination artefacts
  before locking.
  - Requires phase 2.
  - See mpsc-log-design.md §§3-4, 7, 10 and
    terms-of-reference.md §§6-8.
  - Success: simultaneous first writes to a missing directory tree either
    create one usable journal or fail with a specific creation diagnostic.
- [ ] 3.1.2. Implement exclusive journal locking with a configurable timeout.
  - Requires 3.1.1.
  - See mpsc-log-design.md §§2, 4, 6-7, 10 and context.md.
  - Success: contending processes serialize through the same lock file, and
    lock timeout failures return `EX_TEMPFAIL`.
- [ ] 3.1.3. Read sidecar configuration inside the journal critical section.
  - Requires 3.1.2.
  - See mpsc-log-design.md §§4, 6-7 and terms-of-reference.md §8.2.
  - Success: each invocation uses one coherent sidecar view for repair,
    rotation, compression, and append.

### 3.2. Preserve complete records across write and crash-like failures

This step answers whether interrupted or failed writes can leave the journal in
a state the next invocation can recover. It informs the fault-injection adapter
and validates the design's append-plus-repair claim. See mpsc-log-design.md
§§7, 10-12 and terms-of-reference.md §§7-8.

- [ ] 3.2.1. Implement append rollback when a write fails after extending the
  active file.
  - Requires 3.1.2.
  - See mpsc-log-design.md §§7, 10-11.
  - Success: injected write failures truncate the active file back to its
    recorded pre-append length before returning an error.
- [ ] 3.2.2. Implement partial-tail repair before every append.
  - Requires 3.2.1.
  - See mpsc-log-design.md §§6-7, 11.
  - Success: malformed trailing bytes and unterminated final records are
    removed before a new valid record is appended.
- [ ] 3.2.3. Build the fault-injection filesystem coverage for write, truncate,
  rename, compression, and metadata failures.
  - Requires 1.2.3, 3.2.1, and 3.2.2.
  - See mpsc-log-design.md §§7, 10-12.
  - Success: each documented filesystem failure class has a deterministic
    assertion for journal state and exit-code mapping.

### 3.3. Demonstrate concurrent append correctness end to end

This step answers whether many real processes can use `mpsc-log` at once and
produce one complete record per successful command. It informs release
readiness for the first agent-workflow adoption. See mpsc-log-design.md §11
and terms-of-reference.md §§5, 7.

- [ ] 3.3.1. Build a multi-process concurrent append stress harness.
  - Requires steps 3.1-3.2.
  - See mpsc-log-design.md §11 and terms-of-reference.md §7.1.
  - Success: a high-contention run produces the same number of complete,
    decodable JSON object lines as successful child processes.
- [ ] 3.3.2. Add pairwise CLI/configuration combination coverage.
  - Requires phase 2 and 3.3.1.
  - See mpsc-log-design.md §11.
  - Success: the suite covers `jo` syntax form, coercion source, object path,
    sidecar default, rotation schedule, rotation state, and lock contention.

## 4. Rotation and retention without record loss

Idea: if rotation, compression, and retention run under the same lock as
append, operators can keep journal files bounded without handing correctness
back to cron jobs or external shell glue.

This phase delivers bounded storage while preserving the phase 3 concurrency
claim. It treats size-only rotation and scheduled rotation as user-facing modes
that need separate naming and retention evidence.

### 4.1. Deliver size-only rotation for the default policy

This step answers whether the default `schedule = "none"` policy can rotate
under contention without losing records. It informs the gzip and scheduled
rotation work because both build on the same locked action planner. See
mpsc-log-design.md §§6, 8, 11 and mpsc-log-sidecar.example.toml.

- [ ] 4.1.1. Implement size-threshold detection and numeric generation
  planning.
  - Requires phase 3 and 1.1.4.
  - See mpsc-log-design.md §§6, 8, 11.
  - Success: `run.jsonl` rotates to `run.1.jsonl` after the configured
    threshold while preserving the pending record.
- [ ] 4.1.2. Implement oldest-to-newest size-only rename execution.
  - Requires 4.1.1.
  - See mpsc-log-design.md §8.
  - Success: generations advance without overwriting retained files and
    rotation failures leave the previous readable state intact.
- [ ] 4.1.3. Document size-only rotation and retention in the users' guide.
  - Requires 4.1.2.
  - See mpsc-log-design.md §§6, 8 and terms-of-reference.md §§6-7.
  - Success: users can predict active, plain rotated, and compressed filenames
    from a journal path and sidecar settings.

### 4.2. Compress and retain rotated logs atomically

This step answers whether older generations can be gzipped without introducing
the data-loss window the design rejects. It informs scheduled-mode retention
because both modes rely on atomic gzip output and deletion ordering. See
mpsc-log-design.md §§2, 8, 11 and terms-of-reference.md §§6-7.

- [ ] 4.2.1. Implement atomic gzip output for rotated generations.
  - Requires 4.1.2.
  - See mpsc-log-design.md §§2, 8.
  - Success: failed compression leaves the source generation in place and
    aborts before appending the pending record.
- [ ] 4.2.2. Implement retention deletion for plain and compressed
  generations.
  - Requires 4.2.1.
  - See mpsc-log-design.md §§6, 8.
  - Success: files beyond `plain_generations + compressed_generations` are
    deleted only after newer retained files are safely in place.

### 4.3. Deliver UTC scheduled rotation with size splits

This step answers whether hourly, daily, and weekly modes rotate at period
boundaries while still splitting busy periods by bytes. It informs the final
operator contract because scheduled rotation is where naming expectations are
most visible. See mpsc-log-design.md §§6, 8, 11,
mpsc-log-sidecar.example.toml, and context.md.

- [ ] 4.3.1. Implement period calculation for `hourly`, `daily`, and
  `weekly` schedules.
  - Requires 1.2.3 and 4.1.1.
  - See mpsc-log-design.md §§6, 8 and context.md.
  - Success: UTC boundaries produce the documented hour, day, and ISO-week
    beginning names without using a `max_age` configuration field.
- [ ] 4.3.2. Implement active-period detection and scheduled boundary
  rotation.
  - Requires 4.3.1.
  - See mpsc-log-design.md §8.
  - Success: the next invocation after a time break archives the previous
    active segment under the period that produced its records.
- [ ] 4.3.3. Implement interim size-split suffixes inside scheduled periods.
  - Requires 4.3.2.
  - See mpsc-log-design.md §8.
  - Success: busy periods produce ordered `.n` suffixes, and final scheduled
    archives use the next suffix when a period already has size splits.
- [ ] 4.3.4. Implement scheduled-mode period retention and compression.
  - Requires 4.2.2 and 4.3.3.
  - See mpsc-log-design.md §8.
  - Success: the newest completed periods remain plain as complete groups,
    older retained periods are gzipped, and expired periods are deleted.

### 4.4. Demonstrate rotation correctness under contention

This step answers whether size-only and scheduled rotation preserve every
successful record when many processes append concurrently. It informs release
readiness because it exercises the highest-risk feature interactions. See
mpsc-log-design.md §11 and terms-of-reference.md §7.

- [ ] 4.4.1. Build the concurrent rotation end-to-end suite.
  - Requires steps 4.1-4.3.
  - See mpsc-log-design.md §11.
  - Success: forced size and scheduled rotations under concurrent writers
    preserve the decoded record count across active, plain, scheduled, and
    compressed files.
- [ ] 4.4.2. Add fixture coverage for failed rotation and compression actions.
  - Requires 4.4.1.
  - See mpsc-log-design.md §§8, 10-11.
  - Success: injected rename, delete, gzip, and commit failures leave a
    readable previous state and emit the documented exit code.

## 5. df12-build telemetry adoption

Idea: if the finished CLI can emit the first df12-build journal taxonomy with
documented sidecar defaults and realistic invocation fixtures, the workflow
can start collecting real-run evidence without `mpsc-log` becoming an analysis
or dashboard product.

This phase connects the generic journalling tool to its first concrete use
case. It packages examples and verification around phase timing, review rounds,
task shape, CodeRabbit waits, audit yield, remediation lanes, and escaped
defects while respecting the non-goals around discovery and analytics.

### 5.1. Make the df12-build event contract executable

This step answers whether the schema, sidecar defaults, and example event names
are specific enough for the ODW workflow to call. It informs workflow adoption
and future telemetry analysis outside this crate. See mpsc-log-design.md §9,
mpsc-log-event-schema.json, mpsc-log-sidecar.example.toml, and
terms-of-reference.md §§2, 5-7.

- [ ] 5.1.1. Add fixture invocations for the initial df12-build event names.
  - Requires phase 4.
  - See mpsc-log-design.md §9 and terms-of-reference.md §§2, 5-7.
  - Success: fixtures cover `phase.started`, `phase.finished`,
    `review.round`, `task.finished`, `coderabbit.attempt`, `audit.finding`,
    and `defect.escape`.
- [ ] 5.1.2. Validate fixture output against the JSON Schema contract.
  - Requires 5.1.1.
  - See mpsc-log-design.md §9 and mpsc-log-event-schema.json.
  - Success: generated fixture records satisfy required fields, reserved
    namespaces, integer ranges, boolean coercions, and RFC 3339 timestamps.
- [ ] 5.1.3. Provide a df12-build sidecar configuration example.
  - Requires 5.1.2.
  - See mpsc-log-design.md §§6, 9 and
    mpsc-log-sidecar.example.toml.
  - Success: the example sidecar coerces the telemetry fields needed for the
    first workflow integration without imposing organization-wide schema
    governance.

### 5.2. Document operator-facing adoption without expanding scope

This step answers whether workflow authors and operators can use the tool
correctly without reading implementation code. It informs v1 release readiness
and keeps deferred analysis work out of the core CLI. See
terms-of-reference.md §§3-7 and mpsc-log-design.md §§3, 9-10.

- [ ] 5.2.1. Update the users' guide with complete operational examples.
  - Requires 5.1.3.
  - See mpsc-log-design.md §§5-10 and terms-of-reference.md §§4-7.
  - Success: the guide covers simple append, sidecar defaults, coercion
    errors, lock timeout, size rotation, scheduled rotation, and df12-build
    event examples.
- [ ] 5.2.2. Add troubleshooting guidance for agent callers.
  - Requires 5.2.1.
  - See mpsc-log-design.md §§7, 10-11 and terms-of-reference.md §§7-8.
  - Success: operators can distinguish invalid arguments, malformed sidecars,
    timeout, directory creation failure, partial-tail repair, and rotation
    failure from command output and exit status.
- [ ] 5.2.3. Add a release smoke script for the documented workflows.
  - Requires 5.2.1 and 5.2.2.
  - See mpsc-log-design.md §§5-11.
  - Success: the smoke script exercises documented commands against temporary
    journals and fails if examples drift from the binary.

## 6. Deferred extensions after the core v1 promise

Idea: if the core v1 promise is already trustworthy and boring to operate, the
project can evaluate broader extensions on their product value instead of
letting them destabilize the main release.

This phase captures work the current ToR and design explicitly defer. These
items should not block v1 unless a later ADR changes the product boundary.

### 6.1. Evaluate broader observability and analysis features

This step keeps dashboards, queries, and automatic tuning out of the core CLI
while preserving them as possible downstream products. See
terms-of-reference.md §§6.2, 7.3 and mpsc-log-design.md §3.2.

- [ ] 6.1.1. Decide whether query, dashboard, or recommendation tooling belongs
  in a separate crate or repository.
  - Requires phase 5.
  - See terms-of-reference.md §§6.2, 7.3 and mpsc-log-design.md §3.2.
  - Success: the decision does not add query or dashboard obligations to the
    append-only CLI.
- [ ] 6.1.2. Decide whether df12-build analysis should consume journal output
  as a downstream tool.
  - Requires 6.1.1.
  - See terms-of-reference.md §§2, 6.2, 7.3 and mpsc-log-design.md §9.
  - Success: any tuning or reporting work has a separate owner and does not
    change the v1 write contract.

### 6.2. Evaluate expanded platform and compatibility promises

This step keeps distributed coordination, full `jo` parity, public library
APIs, and `max_age` out of v1 while identifying the decisions required if they
become valuable later. See terms-of-reference.md §§6.2, 8-9 and
mpsc-log-design.md §§3.2, 11-13.

- [ ] 6.2.1. Decide whether network-filesystem support should graduate from
  caveat to tested platform promise.
  - Requires phase 5.
  - See mpsc-log-design.md §§7, 11 and terms-of-reference.md §§6.2, 8.2.
  - Success: the project either publishes a named filesystem verification
    matrix or leaves the unsupported-network-filesystem caveat intact.
- [ ] 6.2.2. Decide whether full `jo` compatibility has enough value to expand
  the object-root subset.
  - Requires phase 5.
  - See mpsc-log-design.md §§2, 5, 13 and terms-of-reference.md §§6.2, 9.
  - Success: unsupported `jo` behaviours stay rejected unless an ADR explains
    the compatibility gain and object-root implications.
- [ ] 6.2.3. Decide whether to expose a supported Rust library API.
  - Requires phase 5.
  - See mpsc-log-design.md §§3.2, 12-13 and terms-of-reference.md §9.
  - Success: public API compatibility remains out of v1 unless an ADR defines
    supported types, versioning, and documentation obligations.
- [ ] 6.2.4. Decide whether retention by age should be added after v1.
  - Requires phase 5.
  - See mpsc-log-design.md §§6, 8 and terms-of-reference.md §8.1.
  - Success: `max_age` remains absent unless a later design defines its
    interaction with size, schedule, compression, and concurrent writers.

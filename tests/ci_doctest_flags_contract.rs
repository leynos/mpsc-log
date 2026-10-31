//! Contract that the CI doctest step restates the build standard's flags.
//!
//! `cargo test --doc` is not a `cargo nextest` run, so its step assigns its own
//! `RUSTFLAGS`, and an assigned `RUSTFLAGS` replaces every `rustflags` source in
//! `.cargo/config.toml`. The step therefore has to name the warning policy, the
//! parallel frontend and the Linux linker itself. The workflow is read at
//! compile time, so the test needs no filesystem access.

use rstest::rstest;

const CI: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.github/workflows/ci.yml"
));

/// The command that identifies the doctest step.
const DOCTEST_COMMAND: &str = "cargo test --doc --workspace --all-features";

/// The frontend flag and the Linux linker flag the step must carry.
const THREADS_FLAG: &str = "-Zthreads=8";
const MOLD_FLAG: &str = "-Clink-arg=-fuse-ld=mold";

/// Returns the number of leading spaces on a line.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns whether a line opens a step: a `- ` list item.
fn opens_step(line: &str) -> bool { line.trim_start().starts_with("- ") }

/// Returns the lines of every step in `workflow` whose lines hold `command`, from
/// the step's own list item to the line before the next step.
fn steps_running<'a>(workflow: &'a str, command: &str) -> Vec<Vec<&'a str>> {
    let lines: Vec<&str> = workflow.lines().collect();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if !line.contains(command) || line.trim_start().starts_with('#') {
            continue;
        }
        let Some(start) = (0..=at).rev().find(|index| {
            lines
                .get(*index)
                .is_some_and(|l| opens_step(l) && indent(l) < indent(line))
        }) else {
            continue;
        };
        let step_indent = lines.get(start).map_or(0, |l| indent(l));
        let end = (at + 1..lines.len())
            .find(|index| {
                lines.get(*index).is_some_and(|l| {
                    !l.trim().is_empty()
                        && (indent(l) < step_indent || (opens_step(l) && indent(l) <= step_indent))
                })
            })
            .unwrap_or(lines.len());
        found.push(lines.get(start..end).unwrap_or_default().to_vec());
    }
    found
}

/// Returns the complaint about the doctest step in `workflow`, if any: there must
/// be exactly one, it must assign `RUSTFLAGS` itself, and the value must deny
/// warnings and name the frontend and linker flags as whole words.
fn doctest_problem(workflow: &str) -> Option<String> {
    let steps = steps_running(workflow, DOCTEST_COMMAND);
    let [step] = steps.as_slice() else {
        return Some(format!("expected one doctest step, found {}", steps.len()));
    };
    let assigned: Vec<&str> = step
        .iter()
        .copied()
        .filter(|line| line.trim_start().starts_with("RUSTFLAGS:"))
        .collect();
    let [line] = assigned.as_slice() else {
        return Some(format!(
            "the doctest step must assign RUSTFLAGS once: {step:?}"
        ));
    };
    let words: Vec<&str> = line
        .trim_start()
        .trim_start_matches("RUSTFLAGS:")
        .split_whitespace()
        .take_while(|word| !word.starts_with('#'))
        .map(|word| word.trim_matches(['"', '\'']))
        .collect();
    let denies =
        words.contains(&"-Dwarnings") || words.windows(2).any(|pair| pair == ["-D", "warnings"]);
    let named = |flag: &str| words.contains(&flag);
    if !denies {
        Some(format!("the doctest step stops denying warnings: {line}"))
    } else if !named(THREADS_FLAG) {
        Some(format!("the doctest step loses {THREADS_FLAG}: {line}"))
    } else if !named(MOLD_FLAG) {
        Some(format!("the doctest step loses {MOLD_FLAG}: {line}"))
    } else {
        None
    }
}

/// Scenario: this repository's own CI workflow.
///
/// Invariant: its doctest step assigns `RUSTFLAGS` with the warning policy, the
/// parallel frontend and the Linux linker.
#[test]
fn the_ci_doctest_step_restates_the_standards_flags() {
    assert_eq!(doctest_problem(CI), None);
}

/// A doctest step with the given `RUSTFLAGS` line, between two other steps.
fn workflow_with(env_line: &str) -> String {
    format!(
        "jobs:\n  t:\n    steps:\n      - name: Before\n        env:\n          RUSTFLAGS: -D warnings {THREADS_FLAG} {MOLD_FLAG}\n        run: cargo build\n      - name: Run doctests\n        env:\n{env_line}        run: {DOCTEST_COMMAND}\n      - name: After\n        env:\n          RUSTFLAGS: -D warnings {THREADS_FLAG} {MOLD_FLAG}\n        run: make lint\n"
    )
}

/// Scenario: doctest steps that keep or lose a flag, between steps that carry all
/// of them.
///
/// Invariant: only a step that assigns its own `RUSTFLAGS` once, denying warnings
/// and naming both flags as whole words, passes; a neighbouring step's flags never
/// rescue it, and a lookalike or a comment does not count.
#[rstest]
#[case::all_three(
    "          RUSTFLAGS: -D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold\n",
    false
)]
#[case::joined_deny(
    "          RUSTFLAGS: -Dwarnings -Zthreads=8 -Clink-arg=-fuse-ld=mold\n",
    false
)]
#[case::no_assignment("", true)]
#[case::deny_only("          RUSTFLAGS: -D warnings\n", true)]
#[case::no_frontend_flag("          RUSTFLAGS: -D warnings -Clink-arg=-fuse-ld=mold\n", true)]
#[case::no_linker_flag("          RUSTFLAGS: -D warnings -Zthreads=8\n", true)]
#[case::no_deny("          RUSTFLAGS: -Zthreads=8 -Clink-arg=-fuse-ld=mold\n", true)]
#[case::lookalike_frontend_flag(
    "          RUSTFLAGS: -D warnings -Zthreads=80 -Clink-arg=-fuse-ld=mold\n",
    true
)]
#[case::flags_only_in_a_comment(
    "          RUSTFLAGS: -D warnings # -Zthreads=8 -Clink-arg=-fuse-ld=mold\n",
    true
)]
fn a_doctest_step_must_carry_every_standard_flag(#[case] env_line: &str, #[case] refused: bool) {
    let workflow = workflow_with(env_line);
    assert_eq!(
        doctest_problem(&workflow).is_some(),
        refused,
        "workflow:\n{workflow}"
    );
}

/// Scenario: workflows with no doctest step, or two.
///
/// Invariant: exactly one step must run the doctest command, so a rule that finds
/// nothing cannot pass over an empty workflow.
#[rstest]
#[case::none(0)]
#[case::two(2)]
fn exactly_one_step_runs_the_doctests(#[case] copies: usize) {
    let step = format!(
        "      - name: Doctests\n        env:\n          RUSTFLAGS: -D warnings {THREADS_FLAG} \
         {MOLD_FLAG}\n        run: {DOCTEST_COMMAND}\n"
    );
    let workflow = format!(
        "jobs:\n  t:\n    steps:\n      - name: Build\n        run: cargo build\n{}",
        step.repeat(copies)
    );
    assert!(
        doctest_problem(&workflow).is_some(),
        "workflow:\n{workflow}"
    );
}

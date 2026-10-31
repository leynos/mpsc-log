//! Holds `.cargo/config.toml` free of a codegen backend while the release
//! builds on stable.
//!
//! The release workflow builds with `cross +stable build --release`, and stable
//! Cargo reads `.cargo/config.toml` like any other Cargo. It refuses a
//! `codegen-backend` key there ("config profile `dev` is not valid") and stops,
//! so a backend selected in that file breaks every release build. The Cranelift
//! selection therefore lives in `tools/dev-fast/config.toml`, which only the
//! development make targets pass with `--config`. The judgement is driven
//! against fixtures first, because a rule exercised only over this repository's
//! own compliant files would pass whether or not it detects anything, and then
//! applied to the real files. Both are read as text so the contract needs no
//! parser dependency.

use rstest::rstest;

const CARGO_CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.cargo/config.toml"));
const RELEASE_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.github/workflows/release.yml"
));

/// Returns the lines of `text` with comments removed and blanks dropped.
///
/// A `#` starts a comment in both TOML and YAML, unless it sits inside a quoted
/// string, where it is part of the value and a later key must still be seen.
fn code_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(without_comment)
        .filter(|line| !line.is_empty())
}

/// Returns a line up to a `#` that starts a comment, ignoring one inside a
/// quoted string, without surrounding space.
fn without_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '#') => return line.get(..index).unwrap_or(line).trim(),
            _ => {}
        }
    }
    line.trim()
}

/// Splits a command line at `&&`, `||` and `;`, outside quotes, so each command is read alone: an
/// `echo` that names a toolchain and the build that follows it are two commands.
fn shell_commands(line: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(open), _) => {
                current.push(c);
                if c == open {
                    quote = None;
                }
            }
            (None, '"' | '\'') => {
                current.push(c);
                quote = Some(c);
            }
            (None, ';') => commands.push(std::mem::take(&mut current)),
            (None, '&' | '|') if chars.peek() == Some(&c) => {
                chars.next();
                commands.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    commands.push(current);
    commands
}

/// Returns whether a word precedes the program in a command: a YAML list marker or `run:` key, a
/// block indicator, `sudo`, or an environment assignment.
fn leads_the_program(word: &str) -> bool {
    matches!(word, "-" | "run:" | "|" | ">" | ">-" | "sudo") || word.contains('=')
}

/// Returns the toolchain override (`+stable`, `+nightly-2026-05-28`) one command gives Cargo or
/// `cross` as its first argument, without the `+`.
///
/// Only that shape counts: `+stable` in an `echo`, a URL or a comment is not a build, and the
/// program must be the command itself, not an argument of another command, or reading it would
/// report a key the release never reads.
fn command_override(command: &str) -> Option<String> {
    let mut words = command
        .split_whitespace()
        .skip_while(|word| leads_the_program(word));
    let name = words.next()?.rsplit('/').next().unwrap_or_default();
    if !matches!(name, "cargo" | "cross") {
        return None;
    }
    words.next()?.strip_prefix('+').map(str::to_owned)
}

/// Returns whether the workflow builds on the stable toolchain.
///
/// The toolchain override on the build command decides when the workflow gives
/// one, because that is the toolchain Cargo runs; only a workflow whose commands
/// name none falls back to the `toolchain` a setup action installs.
fn builds_on_stable(workflow: &str) -> bool {
    let overrides: Vec<String> = code_lines(workflow)
        .flat_map(shell_commands)
        .filter_map(|command| command_override(&command))
        .collect();
    if overrides.is_empty() {
        let squeezed = |line: &str| line.replace([' ', '\t', '"', '\''], "");
        return code_lines(workflow).any(|line| squeezed(line) == "toolchain:stable");
    }
    overrides.iter().any(|name| name == "stable")
}

/// Splits a configuration line into its top-level entries, dropping spaces and
/// quote marks: an inline table's entries and its comma-separated pairs, with
/// anything inside a quoted string kept whole.
fn entries(line: &str) -> Vec<String> {
    let mut found = vec![String::new()];
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            // A backslash in a basic string escapes the next character, so `\"` does not end it.
            (Some('"'), '\\') => {
                if let (Some(current), Some(escaped)) = (found.last_mut(), chars.next()) {
                    current.push(escaped);
                }
            }
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '{' | ',') => found.push(String::new()),
            (None, ' ' | '\t') => {}
            _ => {
                if let Some(current) = found.last_mut() {
                    current.push(c);
                }
            }
        }
    }
    found
}

/// Returns whether a configuration line sets a `codegen-backend` key. A string
/// value that merely contains the words, such as an `[env]` entry, is not one.
fn sets_backend_key(line: &str) -> bool {
    entries(line)
        .iter()
        .any(|entry| entry.starts_with("codegen-backend="))
}

/// Returns the configuration lines that select or enable a codegen backend.
///
/// The key is matched wherever it sits in the file, so a nested table such as
/// `[profile.dev.package.foo]` or `[profile.dev.build-override]`, an inline
/// table, and a quoted key are reported as well as `[profile.dev]` and
/// `[unstable]`. Cargo refuses every one of them on stable.
fn backend_keys(config: &str) -> Vec<&str> {
    code_lines(config)
        .filter(|line| sets_backend_key(line))
        .collect()
}

/// Returns the reasons the configuration would break a stable release build.
fn findings(config: &str, release: &str) -> Vec<String> {
    if !builds_on_stable(release) {
        return Vec::new();
    }
    backend_keys(config)
        .into_iter()
        .map(|key| format!("`{key}` is set while the release builds on stable, which refuses it"))
        .collect()
}

/// A release workflow building on stable, as this repository's does.
const STABLE_RELEASE: &str = "steps:\n  - run: cross +stable build --release\n";
/// A release workflow building on the pinned nightly.
const NIGHTLY_RELEASE: &str = "steps:\n  - run: cross +nightly-2026-05-28 build --release\n";
/// A release workflow installing stable through a toolchain action.
const STABLE_TOOLCHAIN_ACTION: &str = concat!(
    "steps:\n  - uses: actions-rust-lang/setup-rust-toolchain@abc\n",
    "    with:\n      toolchain: stable\n  - run: cargo build --release\n"
);
/// A release workflow installing the pinned nightly through a toolchain action.
const NIGHTLY_TOOLCHAIN_ACTION: &str = concat!(
    "steps:\n  - uses: actions-rust-lang/setup-rust-toolchain@abc\n",
    "    with:\n      toolchain: nightly-2026-05-28\n  - run: cargo build --release\n"
);
/// A release workflow that installs stable but builds with a nightly override,
/// so the command, not the action, names the toolchain Cargo runs.
const STABLE_ACTION_NIGHTLY_COMMAND: &str = concat!(
    "steps:\n  - uses: actions-rust-lang/setup-rust-toolchain@abc\n",
    "    with:\n      toolchain: stable\n  - run: cross +nightly-2026-05-28 build --release\n"
);
/// A release workflow installing stable through a toolchain action, with the
/// value quoted.
const STABLE_TOOLCHAIN_QUOTED: &str = concat!(
    "steps:\n  - uses: actions-rust-lang/setup-rust-toolchain@abc\n",
    "    with:\n      toolchain: \"stable\"\n  - run: cargo build --release\n"
);
/// A release workflow that mentions `+stable` in a command that builds nothing.
const STABLE_IN_AN_ECHO: &str =
    "steps:\n  - run: echo +stable is not a toolchain\n  - run: cargo build --release\n";
/// A release workflow invoking Cargo by absolute path on stable.
const STABLE_BY_PATH: &str = "steps:\n  - run: /root/.cargo/bin/cargo +stable build --release\n";
/// A release workflow that mentions stable only in a comment.
const STABLE_IN_A_COMMENT: &str =
    "steps:\n  # Was: cross +stable build --release\n  - run: cross build --release\n";
/// A release workflow whose `echo` names a nightly override before the stable build.
const ECHOED_OVERRIDE_BEFORE_STABLE: &str =
    "steps:\n  - run: echo cross +nightly check && cross +stable build --release\n";
/// A release workflow with two builds, the second on stable.
const TWO_BUILDS_THE_LAST_STABLE: &str =
    "steps:\n  - run: cargo +nightly check; cross +stable build --release\n";
/// A release workflow whose stable build follows an environment assignment.
const STABLE_WITH_ENVIRONMENT: &str =
    "steps:\n  - run: CARGO_TERM_COLOR=always cross +stable build --release\n";
/// The configuration shape a Cranelift default takes.
const CRANELIFT: &str =
    "[unstable]\ncodegen-backend = true\n\n[profile.dev]\ncodegen-backend = \"cranelift\"\n";
/// A configuration with a linker table and a comment naming the key.
const LINKER_ONLY: &str = concat!(
    "# codegen-backend = \"cranelift\" is deliberately absent.\n",
    "[target.x86_64-unknown-linux-gnu]\nlinker = \"clang\"\n"
);

/// Scenario: a configuration with and without a backend key, against release
/// workflows that build on stable or on the pinned nightly.
///
/// Invariant: only a backend key beside a stable release build is reported,
/// once per key, and a comment naming either is not, so the rule is as narrow
/// as it is sufficient.
#[rstest]
#[case::cranelift_with_stable_release(CRANELIFT, STABLE_RELEASE, 2)]
#[case::cranelift_with_stable_toolchain_action(CRANELIFT, STABLE_TOOLCHAIN_ACTION, 2)]
#[case::profile_key_alone("[profile.dev]\ncodegen-backend = \"cranelift\"\n", STABLE_RELEASE, 1)]
#[case::release_profile_key("[profile.release]\ncodegen-backend=\"llvm\"\n", STABLE_RELEASE, 1)]
#[case::nested_package_table(
    "[profile.dev.package.foo]\ncodegen-backend = \"llvm\"\n",
    STABLE_RELEASE,
    1
)]
#[case::build_override_table(
    "[profile.dev.build-override]\ncodegen-backend=\"llvm\"\n",
    STABLE_RELEASE,
    1
)]
#[case::inline_table(
    "profile = { dev = { codegen-backend = \"cranelift\" } }\n",
    STABLE_RELEASE,
    1
)]
#[case::quoted_key(
    "[profile.dev]\n\"codegen-backend\" = \"cranelift\"\n",
    STABLE_RELEASE,
    1
)]
#[case::cargo_invoked_by_path(CRANELIFT, STABLE_BY_PATH, 2)]
#[case::stable_toolchain_action_with_a_quoted_value(CRANELIFT, STABLE_TOOLCHAIN_QUOTED, 2)]
#[case::key_after_a_hash_inside_a_quoted_value(
    "profile = { hint = \"value # text\", codegen-backend = \"cranelift\" }\n",
    STABLE_RELEASE,
    1
)]
#[case::comment_after_the_key(
    "[profile.dev]\ncodegen-backend = \"cranelift\" # a comment\n",
    STABLE_RELEASE,
    1
)]
#[case::inline_table_entry_after_a_comma(
    "profile = { dev = { opt-level = 1, codegen-backend = \"cranelift\" } }\n",
    STABLE_RELEASE,
    1
)]
#[case::string_value_naming_the_key(
    "[env]\nBACKEND_HINT = \"codegen-backend=cranelift\"\n",
    STABLE_RELEASE,
    0
)]
#[case::string_value_holding_a_comma_and_the_key(
    "[env]\nHINT = \"opt-level=1, codegen-backend=cranelift\"\n",
    STABLE_RELEASE,
    0
)]
#[case::stable_action_with_a_nightly_command(CRANELIFT, STABLE_ACTION_NIGHTLY_COMMAND, 0)]
#[case::linker_only_with_stable_release(LINKER_ONLY, STABLE_RELEASE, 0)]
#[case::cranelift_with_nightly_toolchain_action(CRANELIFT, NIGHTLY_TOOLCHAIN_ACTION, 0)]
#[case::stable_named_only_in_an_echo(CRANELIFT, STABLE_IN_AN_ECHO, 0)]
#[case::cranelift_with_nightly_release(CRANELIFT, NIGHTLY_RELEASE, 0)]
#[case::stable_named_only_in_a_comment(CRANELIFT, STABLE_IN_A_COMMENT, 0)]
#[case::an_echoed_override_before_the_stable_build(CRANELIFT, ECHOED_OVERRIDE_BEFORE_STABLE, 2)]
#[case::an_override_on_the_second_of_two_commands(CRANELIFT, TWO_BUILDS_THE_LAST_STABLE, 2)]
#[case::an_escaped_quote_inside_a_value(
    "[env]\nHINT = \"a\\\", codegen-backend=cranelift\"\n",
    STABLE_RELEASE,
    0
)]
#[case::a_key_after_an_escaped_quote_and_a_comma(
    "profile = { hint = \"a\\\" b\", codegen-backend = \"cranelift\" }\n",
    STABLE_RELEASE,
    1
)]
#[case::string_value_with_a_comma_directly_before_the_key(
    "[env]\nHINT = \"opt-level=1,codegen-backend=cranelift\"\n",
    STABLE_RELEASE,
    0
)]
#[case::comment_naming_the_key_after_a_comma(
    "# a note, codegen-backend = \"cranelift\" would break stable\n[profile.dev]\nopt-level = 1\n",
    STABLE_RELEASE,
    0
)]
#[case::an_environment_assignment_before_the_program(CRANELIFT, STABLE_WITH_ENVIRONMENT, 2)]
#[case::tabs_around_the_equals_sign(
    "[profile.dev]\ncodegen-backend\t=\t\"cranelift\"\n",
    STABLE_RELEASE,
    1
)]
fn a_backend_key_is_refused_only_beside_a_stable_release(
    #[case] config: &str,
    #[case] release: &str,
    #[case] expected: usize,
) {
    let found = findings(config, release);
    assert_eq!(found.len(), expected, "saw {found:?}");
}

/// Scenario: this repository's own configuration and release workflow.
///
/// Invariant: the release builds on stable, so `.cargo/config.toml` selects no
/// backend. The first check keeps the rule from passing vacuously should the
/// release move off stable without this contract being revisited.
#[test]
fn the_configuration_selects_no_backend_while_the_release_builds_on_stable() {
    assert!(
        builds_on_stable(RELEASE_WORKFLOW),
        "the release no longer builds on stable; revisit this contract and the Cranelift \
         exception in docs/developers-guide.md"
    );
    let found = findings(CARGO_CONFIG, RELEASE_WORKFLOW);
    assert!(found.is_empty(), "{found:?}");
}

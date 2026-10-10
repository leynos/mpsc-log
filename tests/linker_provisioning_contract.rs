//! Contract test that CI installs clang, lld and mold through `setup-rust`.
//!
//! `.cargo/config.toml` links Linux builds with clang and mold, and coverage
//! links with lld, so the runner needs all three before any cargo command
//! runs. `ci.yml` gets them from the pinned `setup-rust` step's `install-mold`
//! and `install-clang-lld` inputs, which accept only the string `'true'` or
//! `'false'`. A step that lost an input, set it to another value, or went back
//! to an `apt-get` line would surface only as a failed link on a runner, so
//! this suite reads the step and fails first.
//!
//! The reader is textual, like the other workflow contracts here: it takes the
//! `setup-rust` step as the lines after its `uses:` line that are indented
//! deeper, and ignores comments, so a mention of an input in a comment or in
//! another step's script does not satisfy it.

use std::io;

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;

/// The action every provisioning step must use, pinned by commit SHA.
const SETUP_RUST: &str = "leynos/shared-actions/.github/actions/setup-rust@";

/// The two inputs that must each be the string `'true'`.
const LINKER_INPUTS: [&str; 2] = ["install-mold", "install-clang-lld"];

/// Reads `.github/workflows/ci.yml` from the crate root.
fn ci_text() -> io::Result<String> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?
        .read_to_string(".github/workflows/ci.yml")
}

/// Returns the number of leading spaces on a line.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns a step line's key text and the indent of its keys: a leading `- `
/// belongs to the list marker, so the keys sit two columns further in.
fn key_line(line: &str) -> (&str, usize) {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("- ")
        .map_or_else(|| (trimmed, indent(line)), |rest| (rest, indent(line) + 2))
}

/// Returns the lines of the first `setup-rust` step pinned to a full SHA: the
/// `uses:` line and every following line indented as far as its keys, up to
/// the first line indented less. `None` means no pinned step exists.
fn setup_rust_step(workflow: &str) -> Option<Vec<&str>> {
    let lines: Vec<&str> = workflow.lines().collect();
    let start = lines.iter().position(|line| {
        key_line(line)
            .0
            .strip_prefix("uses: ")
            .and_then(|rest| rest.strip_prefix(SETUP_RUST))
            .is_some_and(|reference| {
                let sha = reference.split_whitespace().next().unwrap_or("");
                sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())
            })
    })?;
    let base = key_line(lines.get(start)?).1;
    let mut step = vec![*lines.get(start)?];
    for line in lines.iter().skip(start + 1) {
        if !line.trim().is_empty() && indent(line) < base {
            break;
        }
        step.push(line);
    }
    Some(step)
}

/// Returns the lines of the step's `with:` mapping, which is where the action
/// reads its inputs. A line is in the mapping while it is indented deeper than
/// the `with:` key; an `env:` or `run:` block is a different mapping.
fn with_block<'a>(step: &[&'a str]) -> Vec<&'a str> {
    let base = step.first().map_or(0, |first| key_line(first).1);
    let Some(opening) = step
        .iter()
        .position(|line| line.trim() == "with:" && indent(line) == base)
    else {
        return Vec::new();
    };
    step.iter()
        .skip(opening + 1)
        .take_while(|line| line.trim().is_empty() || indent(line) > base)
        .copied()
        .collect()
}

/// Returns whether the `with:` mapping sets `key` to the string `'true'`.
fn sets_input(inputs: &[&str], key: &str) -> bool {
    let wanted = format!("{key}: 'true'");
    inputs.iter().any(|line| line.trim() == wanted)
}

/// Returns the workflow's shell lines with each backslash continuation joined
/// to the line it continues, so a command split across lines reads as one.
fn logical_lines(workflow: &str) -> Vec<String> {
    let mut joined = Vec::new();
    let mut pending = String::new();
    for line in workflow.lines() {
        let trimmed = line.trim();
        if let Some(head) = trimmed.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
        } else {
            pending.push_str(trimmed);
            joined.push(std::mem::take(&mut pending));
        }
    }
    if !pending.is_empty() {
        joined.push(pending);
    }
    joined
}

/// Returns the commands that apt-install clang, lld or mold by hand, even when
/// the package names sit on a continuation line.
fn hand_installs(workflow: &str) -> Vec<String> {
    logical_lines(workflow)
        .into_iter()
        .filter(|line| !line.starts_with('#'))
        .filter(|line| line.contains("apt-get") && line.contains("install"))
        .filter(|line| {
            line.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|word| matches!(word, "clang" | "lld" | "mold"))
        })
        .collect()
}

/// Checks a workflow, returning what is wrong with its provisioning.
fn problems(workflow: &str) -> Vec<String> {
    let Some(step) = setup_rust_step(workflow) else {
        return vec!["no setup-rust step pinned to a full commit SHA".to_owned()];
    };
    let inputs = with_block(&step);
    let mut found: Vec<String> = LINKER_INPUTS
        .iter()
        .filter(|key| !sets_input(&inputs, key))
        .map(|key| format!("setup-rust does not set {key}: 'true'"))
        .collect();
    found.extend(
        hand_installs(workflow)
            .into_iter()
            .map(|line| format!("hand-rolled install: {line}")),
    );
    found
}

const PIN: &str = "0123456789abcdef0123456789abcdef01234567";

fn fixture(with_block: &str, extra_step: &str) -> String {
    format!(
        "jobs:\n  build-test:\n    steps:\n      - name: Setup Rust\n        uses: \
         {SETUP_RUST}{PIN}\n{with_block}{extra_step}      - name: Next\n        run: make test\n"
    )
}

const BOTH: &str =
    "        with:\n          install-mold: 'true'\n          install-clang-lld: 'true'\n";

#[test]
fn ci_installs_the_linkers_through_setup_rust() {
    let ci = ci_text().expect("ci.yml should be readable");

    assert_eq!(problems(&ci), Vec::<String>::new());
}

#[test]
fn a_step_with_both_inputs_and_no_hand_install_passes() {
    assert_eq!(problems(&fixture(BOTH, "")), Vec::<String>::new());
}

#[rstest]
#[case::no_with_block("", "install-mold")]
#[case::no_clang_lld("        with:\n          install-mold: 'true'\n", "install-clang-lld")]
#[case::no_mold("        with:\n          install-clang-lld: 'true'\n", "install-mold")]
#[case::false_value(
    "        with:\n          install-mold: 'false'\n          install-clang-lld: 'true'\n",
    "install-mold"
)]
#[case::comment_only(
    "        with:\n          # install-mold: 'true'\n          install-clang-lld: 'true'\n",
    "install-mold"
)]
fn a_step_missing_an_input_is_reported(#[case] with_block: &str, #[case] missing: &str) {
    let found = problems(&fixture(with_block, ""));

    assert!(
        found.iter().any(|problem| problem.contains(missing)),
        "expected a problem naming {missing}, got {found:?}"
    );
}

#[test]
fn inputs_under_env_rather_than_with_do_not_count() {
    let under_env =
        "        env:\n          install-mold: 'true'\n          install-clang-lld: 'true'\n";

    assert_eq!(problems(&fixture(under_env, "")).len(), 2);
}

#[test]
fn an_apt_install_split_across_shell_lines_is_reported() {
    let split = "      - name: Install mold linker\n        run: |\n          sudo apt-get \
                 install --yes \\\n            clang lld mold\n";

    let found = problems(&fixture(BOTH, split));

    assert!(
        found
            .iter()
            .any(|problem| problem.starts_with("hand-rolled install")),
        "expected a hand-rolled install, got {found:?}"
    );
}

#[test]
fn an_input_in_another_steps_script_does_not_count() {
    let decoy =
        "      - name: Note\n        run: echo install-mold: 'true' install-clang-lld: 'true'\n";

    assert_eq!(problems(&fixture("", decoy)).len(), 2);
}

#[test]
fn an_unpinned_reference_is_reported() {
    let workflow = format!("      - uses: {SETUP_RUST}main\n");

    assert_eq!(
        problems(&workflow),
        ["no setup-rust step pinned to a full commit SHA"]
    );
}

#[test]
fn a_hand_rolled_apt_install_is_reported_beside_the_inputs() {
    let by_hand = "      - name: Install mold linker\n        run: sudo apt-get install --yes \
                   clang lld mold\n";

    let found = problems(&fixture(BOTH, by_hand));

    assert!(
        found
            .iter()
            .any(|problem| problem.starts_with("hand-rolled install")),
        "expected a hand-rolled install, got {found:?}"
    );
}

#[test]
fn a_commented_apt_install_is_ignored() {
    let note = "      # sudo apt-get install --yes clang lld mold\n";

    assert_eq!(problems(&fixture(BOTH, note)), Vec::<String>::new());
}

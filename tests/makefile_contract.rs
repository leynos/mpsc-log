//! Contract test for the dev-fast Makefile wiring.
//!
//! Guards the sponsor-mandated convention documented in AGENTS.md under
//! "dev-fast is the standard development path": the `build`, `test`,
//! `lint`, and `typecheck` Makefile targets must invoke cargo through
//! `tools/dev-fast/config.toml`, while `coverage` must never do so. An
//! agent editing the Makefile without preserving this wiring fails this
//! suite locally, ahead of the estate-wide DF-004 audit.

use std::io;

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;

/// Opens the crate root as a capability directory, rather than reaching
/// into the ambient filesystem through `std::fs` directly.
///
/// Returns an `io::Error` instead of panicking, so every call site — a
/// `#[test]`/`#[rstest]` function, where `clippy.toml`'s
/// `allow-expect-in-tests` applies — reports the failure with `.expect()`.
fn crate_dir() -> io::Result<Dir> {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
}

/// Reads the repository's `Makefile` from the crate root.
///
/// Returns an `io::Error` instead of panicking; see [`crate_dir`].
fn makefile_text() -> io::Result<String> { crate_dir()?.read_to_string("Makefile") }

/// Returns a rule's indented recipe body: every line after `{target}:` up
/// to the next non-indented line. An empty string means the rule has no
/// recipe of its own — typically a forwarding rule whose commands live in
/// a separate pattern rule.
///
/// # Panics
///
/// Panics if the Makefile has no `{target}:` rule at all, since that means
/// the target was renamed or removed and this test needs updating.
fn recipe_for(makefile: &str, target: &str) -> String {
    let header = format!("{target}:");
    let mut lines = makefile.lines();
    let mut found = false;
    let mut recipe = String::new();
    for line in &mut lines {
        if !found {
            if line.starts_with(&header) {
                found = true;
            }
            continue;
        }
        if line.starts_with('\t') {
            recipe.push_str(line);
            recipe.push('\n');
        } else {
            break;
        }
    }
    assert!(
        found,
        "Makefile has no `{target}:` rule; update this contract test if the target was renamed or \
         removed"
    );
    recipe
}

/// `build`'s cargo invocation lives in the `target/%/$(TARGET)` pattern
/// rule, not directly under `build:`, which is a bare prerequisite
/// forward. Falling back to the pattern rule keeps this assertion honest
/// about where the actual command runs.
fn effective_build_recipe(makefile: &str) -> String {
    let direct = recipe_for(makefile, "build");
    if direct.trim().is_empty() {
        recipe_for(makefile, "target/%/$(TARGET)")
    } else {
        direct
    }
}

// `test_target`, not `test`: rstest 0.26 silently drops the entire case
// list when a case is named `test`, so the label avoids the collision.
#[rstest]
#[case::build("build")]
#[case::test_target("test")]
#[case::lint("lint")]
#[case::typecheck("typecheck")]
fn standard_targets_use_dev_fast(#[case] target: &str) {
    let makefile = makefile_text().expect("failed to read Makefile from the crate root");
    let recipe = if target == "build" {
        effective_build_recipe(&makefile)
    } else {
        recipe_for(&makefile, target)
    };

    assert!(
        recipe.contains("--config"),
        "`{target}`'s recipe must pass --config so cargo picks up the dev-fast fragment; see \
         AGENTS.md, \"dev-fast is the standard development path\""
    );
    let recipe_lower = recipe.to_lowercase();
    assert!(
        recipe_lower.contains("dev-fast") || recipe_lower.contains("dev_fast"),
        "`{target}`'s recipe must reference tools/dev-fast/config.toml (directly or via \
         $(DEV_FAST_CONFIG)); see AGENTS.md, \"dev-fast is the standard development path\""
    );
}

#[test]
fn coverage_target_excludes_dev_fast() {
    let makefile = makefile_text().expect("failed to read Makefile from the crate root");
    let recipe = recipe_for(&makefile, "coverage");
    let recipe_lower = recipe.to_lowercase();
    assert!(
        !recipe_lower.contains("dev-fast") && !recipe_lower.contains("dev_fast"),
        "`coverage` must keep the platform LLVM backend and linker; it must never reference \
         tools/dev-fast/config.toml. See AGENTS.md, \"dev-fast is the standard development path\""
    );
}

#[test]
fn dev_fast_fragment_exists() {
    let dir = crate_dir().expect("failed to open the crate root as a capability directory");
    assert!(
        dir.is_file("tools/dev-fast/config.toml"),
        "tools/dev-fast/config.toml must exist for the dev-fast standard development path \
         documented in AGENTS.md"
    );
}

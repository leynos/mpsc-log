//! Contract for the development build footprint (the estate's rule
//! `rust-dev-build-footprint`): the default development and test profiles keep
//! no incremental state and no debug information, and Cargo coordinates
//! metadata output through its native key rather than a raw rustc flag.
//!
//! The checks ask Cargo itself for the effective values, so a profile override
//! in any configuration file Cargo merges (this repository's, an ancestor
//! directory's or the user's) is seen as well as the manifest's own table.
//! Nothing here compiles: `--unit-graph` and `config get` resolve and print.

use std::process::Command;

/// Profile overrides that the environment can carry; removed so the
/// repository's own settings are what is measured.
const PROFILE_VARIABLES: &[&str] = &[
    "CARGO_INCREMENTAL",
    "CARGO_BUILD_INCREMENTAL",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_DEV_INCREMENTAL",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_INCREMENTAL",
    "CARGO_PROFILE_TEST_DEBUG",
    "CARGO_PROFILE_TEST_INCREMENTAL",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_DEBUG",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_INCREMENTAL",
];

/// Runs the Cargo that built this test in the crate directory and returns its
/// standard output.
fn cargo(args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(env!("CARGO"));
    command.args(args).current_dir(env!("CARGO_MANIFEST_DIR"));
    for name in PROFILE_VARIABLES {
        command.env_remove(name);
    }
    let output = command
        .output()
        .map_err(|error| format!("running cargo {args:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

/// Returns an error carrying the message unless the condition holds.
fn ensure(holds: bool, message: impl FnOnce() -> String) -> Result<(), String> {
    if holds { Ok(()) } else { Err(message()) }
}

/// Returns the profile objects of a `--unit-graph` document: the text after
/// each `"profile":{`, up to the first closing brace.
fn profiles(graph: &str) -> Vec<&str> {
    graph
        .split("\"profile\":{")
        .skip(1)
        .filter_map(|rest| rest.split_once('}').map(|(profile, _)| profile))
        .collect()
}

/// Checks that every unit of the graph is built under the expected profile and disables debug
/// information and incremental state, so a `build.profile` override cannot make a release graph
/// stand in for the development one.
fn check_footprint_profiles(graph: &str, what: &str, expected_profile: &str) -> Result<(), String> {
    let found = profiles(graph);
    ensure(!found.is_empty(), || {
        format!("{what}: the unit graph names no profile")
    })?;
    let expected_name = format!("\"name\":\"{expected_profile}\"");
    for profile in found {
        ensure(profile.contains(&expected_name), || {
            format!("{what}: expected profile {expected_profile}: {profile}")
        })?;
        ensure(profile.contains("\"debuginfo\":0,"), || {
            format!("{what}: a unit keeps debug information: {profile}")
        })?;
        ensure(profile.contains("\"incremental\":false,"), || {
            format!("{what}: a unit keeps incremental state: {profile}")
        })?;
    }
    Ok(())
}

/// Scenario: the unit graph `cargo build` resolves for the development profile.
///
/// Invariant: every unit, the crate's and its dependencies', has debug information off and
/// incremental state off, wherever the setting comes from.
#[test]
fn the_effective_development_profile_disables_incremental_state_and_debug_information()
-> Result<(), String> {
    let graph = cargo(&[
        "build",
        "--unit-graph",
        "-Zunstable-options",
        "--locked",
        "--offline",
    ])?;
    check_footprint_profiles(&graph, "cargo build", "dev")
}

/// Scenario: the unit graph `cargo test --no-run` resolves for the test profile.
///
/// Invariant: the test profile inherits both footprint settings from the development profile, and
/// no configuration file re-enables either.
#[test]
fn the_effective_test_profile_inherits_the_footprint_settings() -> Result<(), String> {
    let graph = cargo(&[
        "test",
        "--no-run",
        "--unit-graph",
        "-Zunstable-options",
        "--locked",
        "--offline",
    ])?;
    check_footprint_profiles(&graph, "cargo test", "test")
}

/// Scenario: the merged Cargo configuration, as `cargo config get` reports it.
///
/// Invariant: `[unstable] embed-metadata` is `false`, the native key for nightlies from 2026-07-31.
#[test]
fn cargo_reads_the_native_key_for_metadata_embedding() -> Result<(), String> {
    let config = cargo(&[
        "config",
        "get",
        "--format",
        "json",
        "-Zunstable-options",
        "unstable.embed-metadata",
        "--locked",
        "--offline",
    ])?;
    ensure(
        config.trim() == r#"{"unstable":{"embed-metadata":false}}"#,
        || {
            format!(
                "[unstable] must set embed-metadata = false on a nightly from 2026-07-31; Cargo \
                 reads {config}"
            )
        },
    )
}

/// Scenario: the `rustflags` Cargo merges for every build and target, as `cargo config get` reports
/// them.
///
/// Invariant: no flag, in a string or an array, joined or split (`-Zembed-metadata=...` or `-Z`
/// then `embed-metadata=...`), in `[build]` or any `[target.*]` table, names metadata embedding,
/// because raw rustc metadata flags broke build scripts and the native key replaces them.
#[test]
fn no_raw_rustc_flag_stands_in_for_the_native_key() -> Result<(), String> {
    for table in ["build", "target"] {
        let reported = cargo(&[
            "config",
            "get",
            "--format",
            "json",
            "-Zunstable-options",
            table,
            "--locked",
            "--offline",
        ])
        .or_else(|error| {
            // A table the configuration does not define is not an error for this contract.
            if error.contains("is not set") {
                Ok(String::new())
            } else {
                Err(error)
            }
        })?;
        ensure(!reported.contains("embed-metadata"), || {
            format!("`{table}` carries a raw metadata flag: {reported}")
        })?;
    }
    Ok(())
}

/// Scenario: the obsolete `no-embed-metadata` key in the repository's `.cargo/config.toml`.
///
/// Invariant: the configuration does not set it, because dated nightlies from 2026-07-31 take
/// `embed-metadata = false` instead.
#[test]
fn the_obsolete_key_is_not_set() -> Result<(), String> {
    let reported = cargo(&[
        "config",
        "get",
        "--format",
        "json",
        "-Zunstable-options",
        "unstable",
        "--locked",
        "--offline",
    ])?;
    ensure(!reported.contains("no-embed-metadata"), || {
        format!("[unstable] still sets the obsolete key: {reported}")
    })
}

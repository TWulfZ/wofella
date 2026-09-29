//! `cargo xtask lint-canary`: runs clippy on `xtask/lint-canary/` and proves the lint policy is
//! live, not just written (spec 001 AC4). Every lint in `BASE_LINTS` and every path in the root
//! `clippy.toml` `disallowed-types` / `disallowed-methods` must fire, and no clippy configuration
//! diagnostic may appear.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use serde::Deserialize;

const BASE_LINTS: [&str; 3] = [
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
];
const DISALLOWED_TYPES: &str = "clippy::disallowed_types";
const DISALLOWED_METHODS: &str = "clippy::disallowed_methods";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Expectation {
    pub(crate) lint: String,
    /// Backtick-quoted path that must appear in the diagnostic, for the path-based lints.
    pub(crate) path: Option<String>,
}

impl std::fmt::Display for Expectation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{} `{path}`", self.lint),
            None => f.write_str(&self.lint),
        }
    }
}

#[derive(Deserialize)]
struct ClippyConfig {
    #[serde(rename = "disallowed-types", default)]
    disallowed_types: Vec<DisallowedPath>,
    #[serde(rename = "disallowed-methods", default)]
    disallowed_methods: Vec<DisallowedPath>,
}

#[derive(Deserialize)]
struct DisallowedPath {
    path: String,
}

#[derive(Deserialize)]
struct CargoLine {
    reason: String,
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Diagnostic {
    code: Option<DiagnosticCode>,
    message: String,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

pub(crate) fn expectations(clippy_toml: &str) -> anyhow::Result<Vec<Expectation>> {
    let config: ClippyConfig = toml::from_str(clippy_toml).context("clippy.toml is malformed")?;
    let mut expected: Vec<Expectation> = BASE_LINTS
        .iter()
        .map(|lint| Expectation {
            lint: (*lint).to_owned(),
            path: None,
        })
        .collect();
    let with_path = |lint: &str, entry: DisallowedPath| Expectation {
        lint: lint.to_owned(),
        path: Some(entry.path),
    };
    expected.extend(
        config
            .disallowed_types
            .into_iter()
            .map(|e| with_path(DISALLOWED_TYPES, e)),
    );
    expected.extend(
        config
            .disallowed_methods
            .into_iter()
            .map(|e| with_path(DISALLOWED_METHODS, e)),
    );
    Ok(expected)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    pub(crate) missing: Vec<Expectation>,
    pub(crate) config_problems: Vec<String>,
}

impl Report {
    fn is_ok(&self) -> bool {
        self.missing.is_empty() && self.config_problems.is_empty()
    }
}

pub(crate) fn evaluate(clippy_json: &str, expected: &[Expectation]) -> anyhow::Result<Report> {
    let mut fired: BTreeSet<(String, String)> = BTreeSet::new();
    let mut config_problems = Vec::new();
    for line in clippy_json.lines().filter(|l| l.starts_with('{')) {
        let parsed: CargoLine =
            serde_json::from_str(line).context("unparsable clippy JSON line")?;
        let Some(diagnostic) = parsed
            .message
            .filter(|_| parsed.reason == "compiler-message")
        else {
            continue;
        };
        match diagnostic.code {
            Some(code) => {
                fired.insert((code.code, diagnostic.message));
            }
            // Clippy reports configuration trouble (unknown clippy.toml key, a disallowed path that
            // does not resolve) as diagnostics without a lint code.
            None if !diagnostic.message.starts_with("aborting due to") => {
                config_problems.push(diagnostic.message);
            }
            None => {}
        }
    }
    let missing = expected
        .iter()
        .filter(|exp| {
            !fired.iter().any(|(code, message)| {
                *code == exp.lint
                    && exp
                        .path
                        .as_ref()
                        .is_none_or(|path| message.contains(&format!("`{path}`")))
            })
        })
        .cloned()
        .collect();
    Ok(Report {
        missing,
        config_problems,
    })
}

pub(crate) fn run(root: &Path) -> anyhow::Result<()> {
    let clippy_toml = std::fs::read_to_string(root.join("clippy.toml"))?;
    let expected = expectations(&clippy_toml)?;
    let canary = root.join("xtask/lint-canary");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(&canary)
        .args(["clippy", "--message-format=json", "--manifest-path"])
        .arg(canary.join("Cargo.toml"))
        // Forces the directory lookup from the canary up to the root clippy.toml, which is what
        // workspace crates rely on.
        .env_remove("CLIPPY_CONF_DIR")
        .env("CARGO_TARGET_DIR", root.join("target/lint-canary"))
        .output()
        .context("spawning cargo clippy on the lint canary")?;
    let stdout = String::from_utf8(output.stdout).context("clippy output is not UTF-8")?;
    let report = evaluate(&stdout, &expected)?;
    if report.is_ok() {
        println!("lint-canary: all {} expected lints fired", expected.len());
        return Ok(());
    }
    for exp in &report.missing {
        println!("lint-canary: did not fire: {exp}");
    }
    for problem in &report.config_problems {
        println!("lint-canary: clippy config problem: {problem}");
    }
    if report.missing.len() == expected.len() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }
    bail!(
        "lint-canary: {} lint(s) missing, {} config problem(s)",
        report.missing.len(),
        report.config_problems.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORDED: &str = include_str!("clippy-canary.ndjson");
    const ROOT_CLIPPY_TOML: &str = include_str!("../../../clippy.toml");

    #[test]
    fn parses_expected_lints_from_json() {
        let expected = expectations(ROOT_CLIPPY_TOML).unwrap();
        assert!(expected.contains(&Expectation {
            lint: DISALLOWED_METHODS.into(),
            path: Some("f64::powf".into()),
        }));
        assert_eq!(
            evaluate(RECORDED, &expected).unwrap(),
            Report::default(),
            "the recorded canary run must satisfy the current clippy.toml"
        );

        // `f64::powf` must not be satisfied by `f64::powi`: paths match with their backticks.
        let without_powf: String = RECORDED
            .lines()
            .filter(|l| !l.contains("`f64::powf`"))
            .map(|l| format!("{l}\n"))
            .collect();
        let report = evaluate(&without_powf, &expected).unwrap();
        assert_eq!(
            report.missing,
            [Expectation {
                lint: DISALLOWED_METHODS.into(),
                path: Some("f64::powf".into()),
            }]
        );

        let without_panic: String = RECORDED
            .lines()
            .filter(|l| !l.contains("clippy::panic"))
            .map(|l| format!("{l}\n"))
            .collect();
        let report = evaluate(&without_panic, &expected).unwrap();
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].lint, "clippy::panic");

        // Recorded from a run with a typo'd path in a scratch clippy.toml.
        let with_config_warning = format!(
            "{RECORDED}{}\n",
            r#"{"reason":"compiler-message","message":{"level":"warning","code":null,"message":"`std::time::SystemTime::nowx` does not refer to a reachable function"}}"#
        );
        let report = evaluate(&with_config_warning, &expected).unwrap();
        assert!(report.missing.is_empty());
        assert_eq!(
            report.config_problems,
            ["`std::time::SystemTime::nowx` does not refer to a reachable function"]
        );

        let empty = evaluate("", &expected).unwrap();
        assert_eq!(empty.missing.len(), expected.len());
    }

    #[test]
    fn canary_lints_match_root() {
        let root: toml::Table = toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
        let canary: toml::Table =
            toml::from_str(include_str!("../../lint-canary/Cargo.toml")).unwrap();
        let root_lints = &root["workspace"]["lints"];
        assert_eq!(&canary["lints"], root_lints);
        assert!(
            canary.contains_key("workspace"),
            "canary must stay a standalone workspace"
        );
    }
}

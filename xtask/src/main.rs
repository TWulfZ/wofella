mod bindings;
mod check_layers;
mod stage_lock;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Repository automation for wolluf")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Check crate edges, banned deps and banned APIs against xtask/layers.toml.
    CheckLayers,
    /// Prove the clippy lint policy fires on xtask/lint-canary.
    LintCanary,
    /// Verify or rewrite stage_versions.lock.
    StageLock {
        #[arg(long)]
        check: bool,
    },
    /// Regenerate the TypeScript IPC bindings through wolluf-desktop.
    Bindings,
}

fn workspace_root() -> PathBuf {
    // xtask always lives one level below the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn run(command: Command) -> anyhow::Result<()> {
    let root = workspace_root();
    match command {
        Command::CheckLayers => check_layers::run(&root),
        Command::LintCanary => anyhow::bail!("xtask lint-canary: not implemented yet (spec 001)"),
        Command::StageLock { check } => stage_lock::run(&root, check),
        Command::Bindings => bindings::run(&root),
    }
}

fn main() -> anyhow::Result<()> {
    run(Cli::parse().command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_all_subcommands() {
        let cases = [
            (vec!["xtask", "check-layers"], Command::CheckLayers),
            (vec!["xtask", "lint-canary"], Command::LintCanary),
            (
                vec!["xtask", "stage-lock"],
                Command::StageLock { check: false },
            ),
            (
                vec!["xtask", "stage-lock", "--check"],
                Command::StageLock { check: true },
            ),
            (vec!["xtask", "bindings"], Command::Bindings),
        ];
        for (argv, expected) in cases {
            assert_eq!(Cli::try_parse_from(argv).unwrap().command, expected);
        }
    }

    #[test]
    fn unimplemented_subcommands_fail_loudly() {
        for command in [Command::LintCanary] {
            let err = run(command).unwrap_err().to_string();
            assert!(err.contains("not implemented"), "{command:?}: {err}");
        }
    }

    #[test]
    fn workspace_root_holds_the_root_manifest() {
        assert!(workspace_root().join("Cargo.toml").is_file());
        assert!(workspace_root().join("xtask/layers.toml").is_file());
    }
}

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

// Skeleton from spec 001 T1; T9–T12 replace each arm. Failing instead of succeeding keeps a CI gate from passing vacuously.
fn run(command: Command) -> anyhow::Result<()> {
    let name = match command {
        Command::CheckLayers => "check-layers",
        Command::LintCanary => "lint-canary",
        Command::StageLock { .. } => "stage-lock",
        Command::Bindings => "bindings",
    };
    anyhow::bail!("xtask {name}: not implemented yet (spec 001)")
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
        for command in [
            Command::CheckLayers,
            Command::LintCanary,
            Command::StageLock { check: true },
            Command::Bindings,
        ] {
            let err = run(command).unwrap_err().to_string();
            assert!(err.contains("not implemented"), "{command:?}: {err}");
        }
    }
}

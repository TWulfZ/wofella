//! `cargo xtask bindings`: delegates to `wolluf-desktop`'s `export-bindings` bin, which writes
//! `apps/desktop/ui/src/ipc/bindings.ts` (ADR 0009). Until spec 005 adds that bin it skips, so
//! the CI drift check can run from day one.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use cargo_metadata::{MetadataCommand, TargetKind};

const DESKTOP_PACKAGE: &str = "wolluf-desktop";
const EXPORT_BIN: &str = "export-bindings";

#[derive(Clone, Debug)]
pub(crate) struct PackageBins {
    pub(crate) name: String,
    pub(crate) bins: Vec<String>,
}

pub(crate) fn has_export_bin(packages: &[PackageBins]) -> bool {
    packages
        .iter()
        .any(|p| p.name == DESKTOP_PACKAGE && p.bins.iter().any(|b| b == EXPORT_BIN))
}

pub(crate) fn run(root: &Path) -> anyhow::Result<()> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cargo metadata failed")?;
    let packages: Vec<PackageBins> = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| PackageBins {
            name: p.name.to_string(),
            bins: p
                .targets
                .iter()
                .filter(|t| t.kind.contains(&TargetKind::Bin))
                .map(|t| t.name.clone())
                .collect(),
        })
        .collect();

    if !has_export_bin(&packages) {
        println!("bindings: skipped ({DESKTOP_PACKAGE} has no {EXPORT_BIN} bin yet, see spec 005)");
        return Ok(());
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["run", "-p", DESKTOP_PACKAGE, "--bin", EXPORT_BIN])
        .status()
        .context("spawning cargo run for export-bindings")?;
    if !status.success() {
        bail!("bindings: {EXPORT_BIN} failed with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, bins: &[&str]) -> PackageBins {
        PackageBins {
            name: name.into(),
            bins: bins.iter().map(|b| (*b).into()).collect(),
        }
    }

    #[test]
    fn skips_without_export_bin() {
        assert!(!has_export_bin(&[]));
        assert!(!has_export_bin(&[
            package("wolluf-desktop", &["wolluf-desktop"]),
            package("wolluf-cli", &["wolluf"]),
        ]));
        // The bin only counts in the desktop shell, which owns the specta command list.
        assert!(!has_export_bin(&[package(
            "wolluf-cli",
            &["export-bindings"]
        )]));
        assert!(has_export_bin(&[package(
            "wolluf-desktop",
            &["wolluf-desktop", "export-bindings"]
        )]));
    }
}

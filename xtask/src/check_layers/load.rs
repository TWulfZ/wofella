use std::path::Path;

use anyhow::Context;
use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand};

use super::{Dep, DepKind, Member, Workspace};

pub(super) fn load(root: &Path) -> anyhow::Result<Workspace> {
    // All features: an optional feature must not be able to smuggle a banned edge past the check.
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()
        .context("cargo metadata failed")?;

    let mut members = Vec::new();
    for package in metadata.workspace_packages() {
        let manifest = std::fs::read_to_string(&package.manifest_path)
            .with_context(|| format!("reading {}", package.manifest_path))?;
        let deps = package
            .dependencies
            .iter()
            .map(|dep| Dep {
                name: dep.name.clone(),
                kind: match dep.kind {
                    DependencyKind::Build => DepKind::Build,
                    DependencyKind::Development => DepKind::Dev,
                    _ => DepKind::Normal,
                },
            })
            .collect();
        members.push(Member {
            name: package.name.to_string(),
            manifest,
            deps,
        });
    }
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { members })
}

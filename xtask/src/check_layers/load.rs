use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand};

use super::{Dep, DepKind, GraphNode, Member, SourceFile, Workspace};

pub(super) fn load(root: &Path) -> anyhow::Result<Workspace> {
    // All features: an optional feature must not be able to smuggle a banned edge past the check.
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()
        .context("cargo metadata failed")?;

    let names: BTreeMap<String, String> = metadata
        .packages
        .iter()
        .map(|p| (p.id.repr.clone(), p.name.to_string()))
        .collect();
    let mut graph = BTreeMap::new();
    for node in metadata.resolve.iter().flat_map(|r| &r.nodes) {
        let deps = node
            .deps
            .iter()
            .filter(|dep| {
                dep.dep_kinds
                    .iter()
                    .any(|info| matches!(info.kind, DependencyKind::Normal | DependencyKind::Build))
            })
            .map(|dep| dep.pkg.repr.clone())
            .collect();
        let name = names
            .get(&node.id.repr)
            .cloned()
            .unwrap_or_else(|| node.id.repr.clone());
        graph.insert(node.id.repr.clone(), GraphNode { name, deps });
    }

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
        let crate_dir = package
            .manifest_path
            .parent()
            .context("manifest path has no parent")?;
        let mut sources = Vec::new();
        collect_sources(crate_dir.as_std_path(), Path::new("src"), &mut sources)?;
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        members.push(Member {
            name: package.name.to_string(),
            id: package.id.repr.clone(),
            manifest,
            deps,
            sources,
        });
    }
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { members, graph })
}

fn collect_sources(
    crate_dir: &Path,
    relative: &Path,
    out: &mut Vec<SourceFile>,
) -> anyhow::Result<()> {
    let dir = crate_dir.join(relative);
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry?;
        let rel = relative.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_sources(crate_dir, &rel, out)?;
        } else if rel.extension().is_some_and(|ext| ext == "rs") {
            let text = std::fs::read_to_string(entry.path())
                .with_context(|| format!("reading {}", entry.path().display()))?;
            // `/` on every OS so violation lines read the same on Windows CI and WSL.
            let path = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push(SourceFile { path, text });
        }
    }
    Ok(())
}

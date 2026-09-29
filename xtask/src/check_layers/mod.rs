//! `cargo xtask check-layers`: enforces architecture §4 (D1, D2, D6, D9) over `cargo metadata`
//! and the member manifests, against the rule data in `xtask/layers.toml`.

mod config;
mod load;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

pub(crate) use config::LayersConfig;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Rule {
    UnlistedCrate,
    ForbiddenEdge,
    LayersCycle,
    NonWorkspaceDep,
    LintsNotInherited,
}

impl Rule {
    fn id(self) -> &'static str {
        match self {
            Self::UnlistedCrate => "L1 unlisted-crate",
            Self::ForbiddenEdge => "L2 forbidden-edge",
            Self::LayersCycle => "L3 layers-cycle",
            Self::NonWorkspaceDep => "L7 non-workspace-dep",
            Self::LintsNotInherited => "L8 lints-not-inherited",
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Violation {
    pub(crate) krate: String,
    pub(crate) rule: Rule,
    pub(crate) detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.krate, self.rule.id(), self.detail)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum DepKind {
    Normal,
    Build,
    Dev,
}

impl fmt::Display for DepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Normal => "normal",
            Self::Build => "build",
            Self::Dev => "dev",
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Dep {
    pub(crate) name: String,
    pub(crate) kind: DepKind,
}

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) manifest: String,
    pub(crate) deps: Vec<Dep>,
}

/// The workspace as the rules see it; built from `cargo metadata` in `load`, by hand in tests.
#[derive(Clone, Debug, Default)]
pub(crate) struct Workspace {
    pub(crate) members: Vec<Member>,
}

pub(crate) fn run(root: &Path) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(root.join("xtask/layers.toml"))?;
    let config = LayersConfig::parse(&config_text)?;
    let workspace = load::load(root)?;
    let violations = check(&config, &workspace);
    for violation in &violations {
        println!("{violation}");
    }
    if violations.is_empty() {
        println!(
            "check-layers: {} members, 0 violations",
            workspace.members.len()
        );
        Ok(())
    } else {
        anyhow::bail!("check-layers: {} violation(s)", violations.len())
    }
}

pub(crate) fn check(config: &LayersConfig, workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    check_layers_cycle(config, &mut violations);
    let internal: BTreeSet<&str> = config
        .crates
        .keys()
        .map(String::as_str)
        .chain(workspace.members.iter().map(|m| m.name.as_str()))
        .collect();
    for member in &workspace.members {
        check_manifest(member, &mut violations);
        let Some(rules) = config.crates.get(&member.name) else {
            violations.push(Violation {
                krate: member.name.clone(),
                rule: Rule::UnlistedCrate,
                detail: "workspace member has no [crates] entry in xtask/layers.toml".into(),
            });
            continue;
        };
        for dep in &member.deps {
            if internal.contains(dep.name.as_str()) && !rules.allowed.contains(&dep.name) {
                violations.push(Violation {
                    krate: member.name.clone(),
                    rule: Rule::ForbiddenEdge,
                    detail: format!(
                        "depends on {} ({}), allowed: [{}]",
                        dep.name,
                        dep.kind,
                        rules.allowed.join(", ")
                    ),
                });
            }
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

fn check_layers_cycle(config: &LayersConfig, violations: &mut Vec<Violation>) {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<'a>(
        node: &'a str,
        config: &'a LayersConfig,
        marks: &mut BTreeMap<&'a str, Mark>,
        stack: &mut Vec<&'a str>,
        cycles: &mut BTreeSet<Vec<&'a str>>,
    ) {
        match marks.get(node) {
            Some(Mark::Done) => return,
            Some(Mark::Visiting) => {
                if let Some(start) = stack.iter().position(|n| *n == node) {
                    let mut cycle = stack[start..].to_vec();
                    // Rotating to the smallest name reports each cycle once, whatever node the DFS entered it from.
                    if let Some(min) = cycle.iter().enumerate().min_by_key(|(_, n)| **n) {
                        let at = min.0;
                        cycle.rotate_left(at);
                    }
                    cycles.insert(cycle);
                }
                return;
            }
            None => {}
        }
        marks.insert(node, Mark::Visiting);
        stack.push(node);
        if let Some(rules) = config.crates.get(node) {
            for next in &rules.allowed {
                visit(next, config, marks, stack, cycles);
            }
        }
        stack.pop();
        marks.insert(node, Mark::Done);
    }

    let mut marks = BTreeMap::new();
    let mut cycles = BTreeSet::new();
    for name in config.crates.keys() {
        visit(name, config, &mut marks, &mut Vec::new(), &mut cycles);
    }
    for cycle in cycles {
        let mut path: Vec<&str> = cycle.clone();
        path.push(cycle[0]);
        violations.push(Violation {
            krate: cycle[0].to_owned(),
            rule: Rule::LayersCycle,
            detail: format!("allowed edges form a cycle: {}", path.join(" -> ")),
        });
    }
}

const DEP_SECTIONS: [&str; 5] = [
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
    "dev_dependencies",
    "build_dependencies",
];

fn check_manifest(member: &Member, violations: &mut Vec<Violation>) {
    let manifest: toml::Table = match toml::from_str(&member.manifest) {
        Ok(table) => table,
        Err(err) => {
            violations.push(Violation {
                krate: member.name.clone(),
                rule: Rule::NonWorkspaceDep,
                detail: format!("manifest is not valid TOML: {err}"),
            });
            return;
        }
    };

    let mut sections: Vec<(String, &toml::Table)> = Vec::new();
    for section in DEP_SECTIONS {
        if let Some(table) = manifest.get(section).and_then(toml::Value::as_table) {
            sections.push((section.to_owned(), table));
        }
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (cfg, target) in targets {
            for section in DEP_SECTIONS {
                if let Some(table) = target.get(section).and_then(toml::Value::as_table) {
                    sections.push((format!("target.{cfg}.{section}"), table));
                }
            }
        }
    }
    for (section, table) in sections {
        for (name, spec) in table {
            let inherited = spec
                .as_table()
                .and_then(|t| t.get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true);
            if !inherited {
                violations.push(Violation {
                    krate: member.name.clone(),
                    rule: Rule::NonWorkspaceDep,
                    detail: format!(
                        "[{section}] {name} is pinned locally; write `{name}.workspace = true` and pin it in the root Cargo.toml"
                    ),
                });
            }
        }
    }

    let lints_inherited = manifest
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    if !lints_inherited {
        violations.push(Violation {
            krate: member.name.clone(),
            rule: Rule::LintsNotInherited,
            detail: "manifest lacks `[lints] workspace = true`".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYERS: &str = r#"
        [crates.wolluf-core]
        layer = "domain"
        allowed = []

        [crates.wolluf-chart]
        layer = "domain"
        allowed = ["wolluf-core"]

        [crates.wolluf-store]
        layer = "adapter"
        allowed = ["wolluf-core"]

        [crates.wolluf-app]
        layer = "app"
        allowed = ["wolluf-core", "wolluf-store"]

        [crates.wolluf-cli]
        layer = "shell"
        allowed = ["wolluf-core", "wolluf-app"]
    "#;

    fn config() -> LayersConfig {
        LayersConfig::parse(LAYERS).unwrap()
    }

    fn manifest(name: &str, deps: &[&str]) -> String {
        let mut text = format!(
            "[package]\nname = \"{name}\"\n\n[lints]\nworkspace = true\n\n[dependencies]\n"
        );
        for dep in deps {
            text.push_str(&format!("{dep}.workspace = true\n"));
        }
        text
    }

    fn member(name: &str, deps: &[&str]) -> Member {
        Member {
            name: name.into(),
            manifest: manifest(name, deps),
            deps: deps
                .iter()
                .map(|d| Dep {
                    name: (*d).into(),
                    kind: DepKind::Normal,
                })
                .collect(),
        }
    }

    fn workspace(members: Vec<Member>) -> Workspace {
        Workspace { members }
    }

    fn clean_workspace() -> Workspace {
        workspace(vec![
            member("wolluf-core", &[]),
            member("wolluf-chart", &["wolluf-core"]),
            member("wolluf-store", &["wolluf-core"]),
            member("wolluf-app", &["wolluf-core", "wolluf-store"]),
            member("wolluf-cli", &["wolluf-core", "wolluf-app", "anyhow"]),
        ])
    }

    fn lines(violations: &[Violation]) -> Vec<String> {
        violations.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn clean_workspace_passes() {
        assert_eq!(
            lines(&check(&config(), &clean_workspace())),
            Vec::<String>::new()
        );
    }

    #[test]
    fn real_layers_toml_is_valid_and_acyclic() {
        let config = LayersConfig::parse(include_str!("../../layers.toml")).unwrap();
        let violations = check(&config, &Workspace::default());
        assert_eq!(lines(&violations), Vec::<String>::new());
        assert!(config.crates.contains_key("xtask"));
    }

    #[test]
    fn rejects_unlisted_crate() {
        let mut ws = clean_workspace();
        ws.members.push(member("wolluf-rogue", &["wolluf-core"]));
        let got = lines(&check(&config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-rogue: L1 unlisted-crate: workspace member has no [crates] entry in xtask/layers.toml"
            ]
        );
    }

    #[test]
    fn rejects_upward_edge() {
        let mut ws = clean_workspace();
        ws.members[0].deps.push(Dep {
            name: "wolluf-chart".into(),
            kind: DepKind::Dev,
        });
        let got = lines(&check(&config(), &ws));
        assert_eq!(
            got,
            ["wolluf-core: L2 forbidden-edge: depends on wolluf-chart (dev), allowed: []"]
        );
    }

    #[test]
    fn rejects_cli_to_store() {
        let mut ws = clean_workspace();
        ws.members[4] = member("wolluf-cli", &["wolluf-core", "wolluf-app", "wolluf-store"]);
        let got = lines(&check(&config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-cli: L2 forbidden-edge: depends on wolluf-store (normal), allowed: [wolluf-core, wolluf-app]"
            ]
        );
    }

    #[test]
    fn rejects_cycle_in_layers_toml() {
        let cyclic = LAYERS.replace(
            "layer = \"domain\"\n        allowed = []",
            "layer = \"domain\"\n        allowed = [\"wolluf-app\"]",
        );
        let got = lines(&check(
            &LayersConfig::parse(&cyclic).unwrap(),
            &Workspace::default(),
        ));
        assert_eq!(
            got,
            [
                "wolluf-app: L3 layers-cycle: allowed edges form a cycle: wolluf-app -> wolluf-core -> wolluf-app"
            ]
        );
    }

    #[test]
    fn rejects_unknown_allowed_target() {
        let typo = LAYERS.replace(
            "allowed = [\"wolluf-core\"]\n\n        [crates.wolluf-store]",
            "allowed = [\"wolluf-cor\"]\n\n        [crates.wolluf-store]",
        );
        let err = LayersConfig::parse(&typo).unwrap_err().to_string();
        assert!(err.contains("unknown crate `wolluf-cor`"), "{err}");
    }

    #[test]
    fn rejects_non_workspace_dep() {
        let mut ws = clean_workspace();
        ws.members[4].manifest = "[package]\nname = \"wolluf-cli\"\n\n[lints]\nworkspace = true\n\n\
             [dependencies]\nanyhow = \"1\"\nwolluf-app = { path = \"../../crates/app\" }\nwolluf-core.workspace = true\n\n\
             [target.'cfg(windows)'.dev-dependencies]\nwinreg = { version = \"0.56\", workspace = false }\n"
            .into();
        let got = lines(&check(&config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-cli: L7 non-workspace-dep: [dependencies] anyhow is pinned locally; write `anyhow.workspace = true` and pin it in the root Cargo.toml",
                "wolluf-cli: L7 non-workspace-dep: [dependencies] wolluf-app is pinned locally; write `wolluf-app.workspace = true` and pin it in the root Cargo.toml",
                "wolluf-cli: L7 non-workspace-dep: [target.cfg(windows).dev-dependencies] winreg is pinned locally; write `winreg.workspace = true` and pin it in the root Cargo.toml",
            ]
        );
    }

    #[test]
    fn rejects_missing_workspace_lints() {
        let mut ws = clean_workspace();
        ws.members[1].manifest =
            "[package]\nname = \"wolluf-chart\"\n\n[dependencies]\nwolluf-core.workspace = true\n"
                .into();
        ws.members[2].manifest =
            "[package]\nname = \"wolluf-store\"\n\n[lints.clippy]\nunwrap_used = \"allow\"\n"
                .into();
        let got = lines(&check(&config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-chart: L8 lints-not-inherited: manifest lacks `[lints] workspace = true`",
                "wolluf-store: L8 lints-not-inherited: manifest lacks `[lints] workspace = true`",
            ]
        );
    }
}

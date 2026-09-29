//! `cargo xtask check-layers`: enforces architecture §4 (D1, D2, D6, D9) over `cargo metadata`
//! and the member manifests, against the rule data in `xtask/layers.toml`.

mod config;
mod load;
mod scan;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

pub(crate) use config::LayersConfig;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Rule {
    UnlistedCrate,
    ForbiddenEdge,
    LayersCycle,
    BannedDep,
    RestrictedDirectDep,
    BannedApi,
    NonWorkspaceDep,
    LintsNotInherited,
}

impl Rule {
    fn id(self) -> &'static str {
        match self {
            Self::UnlistedCrate => "L1 unlisted-crate",
            Self::ForbiddenEdge => "L2 forbidden-edge",
            Self::LayersCycle => "L3 layers-cycle",
            Self::BannedDep => "L4 banned-dep",
            Self::RestrictedDirectDep => "L5 restricted-direct-dep",
            Self::BannedApi => "L6 banned-api",
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
pub(crate) struct SourceFile {
    /// Relative to the crate directory, `/`-separated.
    pub(crate) path: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub(crate) name: String,
    /// Key of this crate in `Workspace::graph`.
    pub(crate) id: String,
    pub(crate) manifest: String,
    pub(crate) deps: Vec<Dep>,
    pub(crate) sources: Vec<SourceFile>,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphNode {
    pub(crate) name: String,
    /// Ids of normal and build dependencies only: dev-dependencies never reach a built artifact.
    pub(crate) deps: Vec<String>,
}

/// The workspace as the rules see it; built from `cargo metadata` in `load`, by hand in tests.
#[derive(Clone, Debug, Default)]
pub(crate) struct Workspace {
    pub(crate) members: Vec<Member>,
    pub(crate) graph: BTreeMap<String, GraphNode>,
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
        check_restricted_deps(config, member, &mut violations);
        let Some(rules) = config.crates.get(&member.name) else {
            violations.push(Violation {
                krate: member.name.clone(),
                rule: Rule::UnlistedCrate,
                detail: "workspace member has no [crates] entry in xtask/layers.toml".into(),
            });
            check_apis(config, None, member, &mut violations);
            continue;
        };
        let layer_rules = config.layer_rules(rules.layer);
        check_banned_tree(&layer_rules.banned_deps, workspace, member, &mut violations);
        for dep in &member.deps {
            if layer_rules.banned_direct_deps.contains(&dep.name) {
                violations.push(Violation {
                    krate: member.name.clone(),
                    rule: Rule::RestrictedDirectDep,
                    detail: format!(
                        "direct dependency on {} ({}) is banned in the {:?} layer",
                        dep.name, dep.kind, rules.layer
                    ),
                });
            }
        }
        check_apis(config, Some(rules), member, &mut violations);
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

fn check_restricted_deps(config: &LayersConfig, member: &Member, violations: &mut Vec<Violation>) {
    for dep in &member.deps {
        if let Some(owners) = config.restricted_deps.get(&dep.name)
            && !owners.contains(&member.name)
        {
            violations.push(Violation {
                krate: member.name.clone(),
                rule: Rule::RestrictedDirectDep,
                detail: format!(
                    "direct dependency on {} ({}); only [{}] may depend on it",
                    dep.name,
                    dep.kind,
                    owners.join(", ")
                ),
            });
        }
    }
}

fn check_banned_tree(
    banned: &[String],
    workspace: &Workspace,
    member: &Member,
    violations: &mut Vec<Violation>,
) {
    if banned.is_empty() {
        return;
    }
    // BFS so each banned package is reported with its shortest path from the member.
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::from([member.id.as_str()]);
    let mut seen = BTreeSet::from([member.id.as_str()]);
    let mut reported = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        let Some(node) = workspace.graph.get(id) else {
            continue;
        };
        if id != member.id && banned.contains(&node.name) && reported.insert(node.name.as_str()) {
            let mut path = vec![node.name.as_str()];
            let mut cursor = id;
            while let Some(prev) = parent.get(cursor) {
                cursor = prev;
                path.push(
                    workspace
                        .graph
                        .get(cursor)
                        .map_or(cursor, |n| n.name.as_str()),
                );
            }
            path.reverse();
            violations.push(Violation {
                krate: member.name.clone(),
                rule: Rule::BannedDep,
                detail: format!(
                    "banned dependency {} in the normal/build tree: {}",
                    node.name,
                    path.join(" -> ")
                ),
            });
        }
        for next in &node.deps {
            if seen.insert(next.as_str()) {
                parent.insert(next.as_str(), id);
                queue.push_back(next.as_str());
            }
        }
    }
}

fn check_apis(
    config: &LayersConfig,
    rules: Option<&config::CrateRules>,
    member: &Member,
    violations: &mut Vec<Violation>,
) {
    let mut api_rules: Vec<&config::ApiRule> = Vec::new();
    if let Some(rules) = rules {
        api_rules.extend(&config.layer_rules(rules.layer).banned_apis);
        api_rules.extend(&rules.banned_apis);
    }
    api_rules.extend(
        config
            .restricted_apis
            .iter()
            .filter(|api| !api.allowed_in.contains(&member.name))
            .map(|api| &api.rule),
    );
    if api_rules.is_empty() {
        return;
    }
    for file in &member.sources {
        let code = scan::code_only(&file.text);
        for rule in &api_rules {
            for found in rule.regex.find_iter(&code) {
                violations.push(Violation {
                    krate: member.name.clone(),
                    rule: Rule::BannedApi,
                    detail: format!(
                        "{} at {}:{}",
                        rule.label,
                        file.path,
                        scan::line_of(&code, found.start())
                    ),
                });
            }
        }
    }
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
            id: name.into(),
            manifest: manifest(name, deps),
            deps: deps
                .iter()
                .map(|d| Dep {
                    name: (*d).into(),
                    kind: DepKind::Normal,
                })
                .collect(),
            sources: Vec::new(),
        }
    }

    /// Graph ids are package names; external packages are added with `package`.
    fn workspace(members: Vec<Member>) -> Workspace {
        let mut ws = Workspace {
            members,
            graph: BTreeMap::new(),
        };
        for m in ws.members.clone() {
            let deps: Vec<&str> = m
                .deps
                .iter()
                .filter(|d| d.kind != DepKind::Dev)
                .map(|d| d.name.as_str())
                .collect();
            package(&mut ws, &m.name, &deps);
        }
        ws
    }

    fn package(ws: &mut Workspace, name: &str, deps: &[&str]) {
        ws.graph.insert(
            name.into(),
            GraphNode {
                name: name.into(),
                deps: deps.iter().map(|d| (*d).into()).collect(),
            },
        );
    }

    fn real_config() -> LayersConfig {
        LayersConfig::parse(include_str!("../../layers.toml")).unwrap()
    }

    fn real_workspace() -> Workspace {
        workspace(vec![
            member("wolluf-core", &[]),
            member("wolluf-chart", &["wolluf-core"]),
            member("wolluf-store", &["wolluf-core"]),
            member("wolluf-source-osu", &["wolluf-core"]),
            member(
                "wolluf-app",
                &["wolluf-core", "wolluf-store", "wolluf-source-osu"],
            ),
            member("wolluf-desktop", &["wolluf-core", "wolluf-app"]),
            member("wolluf-cli", &["wolluf-core", "wolluf-app", "anyhow"]),
            member("xtask", &["anyhow"]),
        ])
    }

    fn add_source(ws: &mut Workspace, krate: &str, path: &str, text: &str) {
        let m = ws.members.iter_mut().find(|m| m.name == krate).unwrap();
        m.sources.push(SourceFile {
            path: path.into(),
            text: text.into(),
        });
    }

    fn add_dep(ws: &mut Workspace, krate: &str, dep: &str, kind: DepKind) {
        let m = ws.members.iter_mut().find(|m| m.name == krate).unwrap();
        m.deps.push(Dep {
            name: dep.into(),
            kind,
        });
        if kind != DepKind::Dev {
            ws.graph.get_mut(krate).unwrap().deps.push(dep.into());
            ws.graph.entry(dep.into()).or_insert_with(|| GraphNode {
                name: dep.into(),
                deps: Vec::new(),
            });
        }
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

    #[test]
    fn real_workspace_fixture_passes_real_rules() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-source-osu",
            "src/read.rs",
            "use std::fs;\npub fn f(p: &std::path::Path) { let _ = fs::read(p); let _ = std::fs::symlink_metadata(p); }\n",
        );
        add_source(
            &mut ws,
            "wolluf-app",
            "src/clock.rs",
            "#[allow(clippy::disallowed_methods)]\nfn now() { std::time::SystemTime::now(); }\n",
        );
        add_source(
            &mut ws,
            "wolluf-store",
            "src/db.rs",
            "use rusqlite::Connection;\n",
        );
        add_dep(&mut ws, "wolluf-store", "rusqlite", DepKind::Normal);
        add_dep(&mut ws, "wolluf-app", "tokio", DepKind::Normal);
        add_dep(&mut ws, "wolluf-core", "proptest", DepKind::Dev);
        assert_eq!(lines(&check(&real_config(), &ws)), Vec::<String>::new());
    }

    #[test]
    fn rejects_tokio_in_domain_tree() {
        let mut ws = real_workspace();
        add_dep(&mut ws, "wolluf-chart", "some-lib", DepKind::Normal);
        package(&mut ws, "some-lib", &["helper"]);
        package(&mut ws, "helper", &["tokio"]);
        package(&mut ws, "tokio", &[]);
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-chart: L4 banned-dep: banned dependency tokio in the normal/build tree: wolluf-chart -> some-lib -> helper -> tokio"
            ]
        );
    }

    #[test]
    fn rejects_rusqlite_outside_store() {
        let mut ws = real_workspace();
        add_dep(&mut ws, "wolluf-app", "rusqlite", DepKind::Dev);
        add_source(
            &mut ws,
            "wolluf-app",
            "src/plays.rs",
            "fn f() {\n    let _c = rusqlite::Connection::open_in_memory();\n}\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-app: L5 restricted-direct-dep: direct dependency on rusqlite (dev); only [wolluf-store] may depend on it",
                "wolluf-app: L6 banned-api: rusqlite:: at src/plays.rs:2",
            ]
        );
    }

    #[test]
    fn rejects_tokio_direct_in_store() {
        let mut ws = real_workspace();
        add_dep(&mut ws, "wolluf-store", "tokio", DepKind::Normal);
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-store: L5 restricted-direct-dep: direct dependency on tokio (normal); only [wolluf-app, wolluf-desktop, wolluf-cli] may depend on it"
            ]
        );
    }

    #[test]
    fn rejects_std_fs_in_domain() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-core",
            "src/lib.rs",
            "pub fn f() {\n    let _ = std::fs::read(\"x\");\n    let _ = ::std :: env::var(\"HOME\");\n    let _ = std::time::SystemTime::now();\n}\n",
        );
        add_source(
            &mut ws,
            "wolluf-chart",
            "src/lib.rs",
            "use std::process::Command;\nfn g() { let _ = std::time::Instant::now(); let _ = rand::thread_rng(); }\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-chart: L6 banned-api: Instant::now at src/lib.rs:2",
                "wolluf-chart: L6 banned-api: std::fs|net|env|process at src/lib.rs:1",
                "wolluf-chart: L6 banned-api: thread_rng at src/lib.rs:2",
                "wolluf-core: L6 banned-api: SystemTime::now at src/lib.rs:4",
                "wolluf-core: L6 banned-api: std::fs|net|env|process at src/lib.rs:2",
                "wolluf-core: L6 banned-api: std::fs|net|env|process at src/lib.rs:3",
            ]
        );
    }

    #[test]
    fn rejects_braced_use_std_fs_in_domain() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-core",
            "src/a.rs",
            "use std::{\n    io::Read,\n    fs,\n};\n",
        );
        add_source(
            &mut ws,
            "wolluf-core",
            "src/b.rs",
            "use std::{collections::BTreeMap, env::var};\n",
        );
        add_source(
            &mut ws,
            "wolluf-core",
            "src/ok.rs",
            "use std::{collections::BTreeMap, fmt};\nuse std::io::{self, Read};\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-core: L6 banned-api: use std::{fs|net|env|process} at src/a.rs:1",
                "wolluf-core: L6 banned-api: use std::{fs|net|env|process} at src/b.rs:1",
            ]
        );
    }

    #[test]
    fn ignores_commented_tokens() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-core",
            "src/lib.rs",
            "//! Never call std::fs here.\n/// Unlike SystemTime::now, a Clock is injected.\n// std::env::var(\"X\")\n/* outer /* nested std::net */ still SystemTime::now */\nconst URL: &str = \"https://example.org/*\"; const C: char = '\"'; fn f<'a>(x: &'a str) -> &'a str { x }\nconst R: &str = r#\"quote \" // not a comment\"#;\nconst M: &str = \"use std::fs or SystemTime::now instead\";\n",
        );
        assert_eq!(lines(&check(&real_config(), &ws)), Vec::<String>::new());

        // Code after a string that contains `//` is still scanned.
        add_source(
            &mut ws,
            "wolluf-chart",
            "src/lib.rs",
            "const U: &str = \"a//b\"; fn f() { let _ = std::fs::read(U); }\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            ["wolluf-chart: L6 banned-api: std::fs|net|env|process at src/lib.rs:1"]
        );
    }

    #[test]
    fn rejects_file_create_in_source_osu() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-source-osu",
            "src/export.rs",
            "use std::fs::{self, write};\nfn f(p: &std::path::Path) {\n    let _ = std::fs::File::create(p);\n    let _ = std::fs::OpenOptions::new();\n    let _ = fs::remove_dir_all(p);\n}\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-source-osu: L6 banned-api: File::create at src/export.rs:3",
                "wolluf-source-osu: L6 banned-api: OpenOptions at src/export.rs:4",
                "wolluf-source-osu: L6 banned-api: remove_dir at src/export.rs:5",
                "wolluf-source-osu: L6 banned-api: use std::fs::{write|rename|copy} at src/export.rs:1",
            ]
        );
    }

    #[test]
    fn rejects_allow_disallowed_in_domain() {
        let mut ws = real_workspace();
        add_source(
            &mut ws,
            "wolluf-chart",
            "src/lib.rs",
            "#[allow(clippy::disallowed_types)]\nfn f() {}\n#[expect( clippy :: disallowed_methods, reason = \"x\")]\nfn g() {}\n",
        );
        add_source(
            &mut ws,
            "wolluf-cli",
            "src/main.rs",
            "#[allow(clippy::disallowed_types)]\nfn f() {}\n",
        );
        let got = lines(&check(&real_config(), &ws));
        assert_eq!(
            got,
            [
                "wolluf-chart: L6 banned-api: allow(clippy::disallowed_*) at src/lib.rs:1",
                "wolluf-chart: L6 banned-api: allow(clippy::disallowed_*) at src/lib.rs:3",
            ]
        );
    }
}

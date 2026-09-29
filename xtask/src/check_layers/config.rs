#![expect(
    dead_code,
    reason = "L4-L6 rule data is parsed now and enforced by 001-T10"
)]

use std::collections::BTreeMap;

use anyhow::{Context, bail};
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Layer {
    Domain,
    Engine,
    Adapter,
    App,
    Shell,
    Tool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    layers: BTreeMap<Layer, RawLayerRules>,
    crates: BTreeMap<String, RawCrateRules>,
    #[serde(default)]
    restricted_deps: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    restricted_apis: Vec<RawRestrictedApi>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLayerRules {
    #[serde(default)]
    banned_deps: Vec<String>,
    #[serde(default)]
    banned_direct_deps: Vec<String>,
    #[serde(default)]
    banned_apis: Vec<RawApiRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCrateRules {
    layer: Layer,
    allowed: Vec<String>,
    #[serde(default)]
    banned_apis: Vec<RawApiRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApiRule {
    label: String,
    regex: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRestrictedApi {
    label: String,
    regex: String,
    allowed_in: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct ApiRule {
    pub(crate) label: String,
    pub(crate) regex: Regex,
}

#[derive(Debug, Default)]
pub(crate) struct LayerRules {
    pub(crate) banned_deps: Vec<String>,
    pub(crate) banned_direct_deps: Vec<String>,
    pub(crate) banned_apis: Vec<ApiRule>,
}

#[derive(Debug)]
pub(crate) struct CrateRules {
    pub(crate) layer: Layer,
    pub(crate) allowed: Vec<String>,
    pub(crate) banned_apis: Vec<ApiRule>,
}

#[derive(Debug)]
pub(crate) struct RestrictedApi {
    pub(crate) rule: ApiRule,
    pub(crate) allowed_in: Vec<String>,
}

/// Parsed and validated `xtask/layers.toml`.
#[derive(Debug)]
pub(crate) struct LayersConfig {
    pub(crate) layers: BTreeMap<Layer, LayerRules>,
    pub(crate) crates: BTreeMap<String, CrateRules>,
    pub(crate) restricted_deps: BTreeMap<String, Vec<String>>,
    pub(crate) restricted_apis: Vec<RestrictedApi>,
}

static NO_RULES: LayerRules = LayerRules {
    banned_deps: Vec::new(),
    banned_direct_deps: Vec::new(),
    banned_apis: Vec::new(),
};

impl LayersConfig {
    pub(crate) fn parse(text: &str) -> anyhow::Result<Self> {
        let raw: RawConfig = toml::from_str(text).context("layers.toml is malformed")?;

        let mut crates = BTreeMap::new();
        for (name, rules) in raw.crates {
            let banned_apis = compile_all(rules.banned_apis)
                .with_context(|| format!("layers.toml: crates.{name}.banned_apis"))?;
            crates.insert(
                name,
                CrateRules {
                    layer: rules.layer,
                    allowed: rules.allowed,
                    banned_apis,
                },
            );
        }
        // A typo in an allowed list would silently permit nothing, or hide a real edge, so unknown names are fatal.
        for (name, rules) in &crates {
            for target in &rules.allowed {
                if !crates.contains_key(target) {
                    bail!("layers.toml: crates.{name}.allowed names unknown crate `{target}`");
                }
            }
        }
        for (dep, owners) in &raw.restricted_deps {
            check_known(&crates, owners, &format!("restricted_deps.{dep}"))?;
        }

        let mut layers = BTreeMap::new();
        for (layer, rules) in raw.layers {
            let banned_apis = compile_all(rules.banned_apis)
                .with_context(|| format!("layers.toml: layers.{layer:?}.banned_apis"))?;
            layers.insert(
                layer,
                LayerRules {
                    banned_deps: rules.banned_deps,
                    banned_direct_deps: rules.banned_direct_deps,
                    banned_apis,
                },
            );
        }

        let mut restricted_apis = Vec::new();
        for api in raw.restricted_apis {
            check_known(
                &crates,
                &api.allowed_in,
                &format!("restricted_apis `{}`", api.label),
            )?;
            restricted_apis.push(RestrictedApi {
                rule: compile(RawApiRule {
                    label: api.label,
                    regex: api.regex,
                })?,
                allowed_in: api.allowed_in,
            });
        }

        Ok(Self {
            layers,
            crates,
            restricted_deps: raw.restricted_deps,
            restricted_apis,
        })
    }

    pub(crate) fn layer_rules(&self, layer: Layer) -> &LayerRules {
        self.layers.get(&layer).unwrap_or(&NO_RULES)
    }
}

fn check_known(
    crates: &BTreeMap<String, CrateRules>,
    names: &[String],
    what: &str,
) -> anyhow::Result<()> {
    for name in names {
        if !crates.contains_key(name) {
            bail!("layers.toml: {what} names unknown crate `{name}`");
        }
    }
    Ok(())
}

fn compile_all(rules: Vec<RawApiRule>) -> anyhow::Result<Vec<ApiRule>> {
    rules.into_iter().map(compile).collect()
}

fn compile(rule: RawApiRule) -> anyhow::Result<ApiRule> {
    let regex = Regex::new(&rule.regex)
        .with_context(|| format!("invalid regex for banned API `{}`", rule.label))?;
    Ok(ApiRule {
        label: rule.label,
        regex,
    })
}

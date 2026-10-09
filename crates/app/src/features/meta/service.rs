//! `MetaService`: the meta feature's only entry point for shells (D12).

use wolluf_engine::profile::Registry;

use super::dto::KeymodeDto;

/// Holds no state: everything it reports is compiled into the engine.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetaService;

impl MetaService {
    /// Enabled keymodes, ascending.
    pub fn keymodes(&self) -> Vec<KeymodeDto> {
        Registry::builtin()
            .profiles()
            .iter()
            .map(|p| KeymodeDto {
                keymode: p.keymode.columns(),
                has_patterns: p.taxonomy.is_some_and(|t| !t.is_empty()),
                calculators: p
                    .calculators
                    .iter()
                    .map(|c| c.as_str().to_owned())
                    .collect(),
                default_layout: p.layout().id().to_owned(),
                has_thumb: p.has_thumb(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::meta::dto::KeymodeDto;

    #[test]
    fn keymodes_come_from_the_engine_registry() {
        let keymodes = MetaService.keymodes();
        assert_eq!(
            keymodes,
            [
                KeymodeDto {
                    keymode: 4,
                    has_patterns: false,
                    calculators: vec!["minacalc".to_owned()],
                    default_layout: "k4.generic".to_owned(),
                    has_thumb: false,
                },
                KeymodeDto {
                    keymode: 7,
                    has_patterns: true,
                    calculators: vec!["minacalc".to_owned()],
                    default_layout: "k7.313_right_thumb".to_owned(),
                    has_thumb: true,
                },
            ]
        );
    }

    #[test]
    fn keymode_dto_is_camel_case_on_the_wire() {
        let json = serde_json::to_string(&MetaService.keymodes()[0]).unwrap();
        assert_eq!(
            json,
            r#"{"keymode":4,"hasPatterns":false,"calculators":["minacalc"],"defaultLayout":"k4.generic","hasThumb":false}"#
        );
    }
}

//! Session-user auto-selection (spec 004 R5–R7, ADR 0005 amendment). Pure and IO-free.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use wolluf_core::AliasId;

use super::IdentityParams;
use super::names::{SessionMatchKind, norm_len, normalize, session_match};

/// Returned in the DTO; bump when the same inputs would select differently.
pub const SELECTION_VERSION: u32 = 1;

macro_rules! stable_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

stable_enum!(
    /// The user's persisted answer (`identity_decision`); stable strings, never renumbered.
    Decision { Me => "me", NotMe => "not_me" }
);

stable_enum!(
    /// Which login an alias matched. `LinkedAccount` is F3 (osu! `/me`); always absent in F0.
    MatchSource { CfgUsername => "cfg_username", LinkedAccount => "linked_account" }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoMatch {
    pub source: MatchSource,
    pub kind: SessionMatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasFacts {
    pub alias_id: AliasId,
    /// Raw bytes as osu! stored them: the alias key (§5.3); normalization is for matching only.
    pub raw_name: Vec<u8>,
    pub n_plays: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInputs {
    pub aliases: Vec<AliasFacts>,
    pub cfg_username: Option<String>,
    pub linked_username: Option<String>,
    pub decisions: BTreeMap<AliasId, Decision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasSelection {
    pub alias_id: AliasId,
    pub norm_len: u32,
    /// Reported even when a decision overrides it, so the row keeps its chip and its place.
    pub auto_match: Option<AutoMatch>,
    pub decision: Option<Decision>,
    pub selected: bool,
}

/// Rows come back auto-matched first, then by play count descending, then by raw name bytes,
/// whatever the input order (R6). A decision always wins over the auto rule (R7).
pub fn select(inputs: &SelectionInputs, params: &IdentityParams) -> Vec<AliasSelection> {
    let logins: Vec<(MatchSource, String)> = [
        (MatchSource::CfgUsername, &inputs.cfg_username),
        (MatchSource::LinkedAccount, &inputs.linked_username),
    ]
    .into_iter()
    .filter_map(|(source, login)| login.as_deref().map(|l| (source, normalize(l))))
    .collect();

    let mut rows: Vec<(&AliasFacts, AliasSelection)> = inputs
        .aliases
        .iter()
        .map(|facts| {
            let norm = normalize(&String::from_utf8_lossy(&facts.raw_name));
            let auto_match = logins.iter().find_map(|(source, login)| {
                session_match(&norm, login, params).map(|kind| AutoMatch {
                    source: *source,
                    kind,
                })
            });
            let decision = inputs.decisions.get(&facts.alias_id).copied();
            let selected = match decision {
                Some(Decision::Me) => true,
                Some(Decision::NotMe) => false,
                None => auto_match.is_some(),
            };
            let selection = AliasSelection {
                alias_id: facts.alias_id,
                norm_len: norm_len(&norm),
                auto_match,
                decision,
                selected,
            };
            (facts, selection)
        })
        .collect();

    rows.sort_by(|(fa, a), (fb, b)| {
        (
            a.auto_match.is_none(),
            Reverse(fa.n_plays),
            &fa.raw_name,
            fa.alias_id,
        )
            .cmp(&(
                b.auto_match.is_none(),
                Reverse(fb.n_plays),
                &fb.raw_name,
                fb.alias_id,
            ))
    });
    rows.into_iter().map(|(_, selection)| selection).collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::features::players::names::{SessionMatchKind, normalize};
    use crate::features::players::testkit::{pilot_alias_id, pilot_like_fixture};

    fn raw_name_of(inputs: &SelectionInputs, id: AliasId) -> String {
        let facts = inputs.aliases.iter().find(|a| a.alias_id == id).unwrap();
        String::from_utf8(facts.raw_name.clone()).unwrap()
    }

    fn cfg_match(kind: SessionMatchKind) -> Option<AutoMatch> {
        Some(AutoMatch {
            source: MatchSource::CfgUsername,
            kind,
        })
    }

    #[test]
    fn pilot_like_selection() {
        let inputs = pilot_like_fixture();
        let out = select(&inputs, &IdentityParams::default());
        let rows: Vec<(String, Option<AutoMatch>, bool)> = out
            .iter()
            .map(|s| (raw_name_of(&inputs, s.alias_id), s.auto_match, s.selected))
            .collect();
        let expected = vec![
            (
                "TWulfZ".to_owned(),
                cfg_match(SessionMatchKind::Prefix),
                true,
            ),
            (
                "TWulfZasdasdasd d jSS||".to_owned(),
                cfg_match(SessionMatchKind::Equal),
                true,
            ),
            (String::new(), None, false),
            ("W".to_owned(), None, false),
            ("Rosalind".to_owned(), None, false),
            ("s".to_owned(), None, false),
            ("w".to_owned(), None, false),
            ("Kovacs".to_owned(), None, false),
            ("Wulf".to_owned(), None, false),
            ("Sterling".to_owned(), None, false),
        ];
        assert_eq!(rows, expected);
        assert!(out.iter().all(|s| s.decision.is_none()));
        let twulfz = out
            .iter()
            .find(|s| s.alias_id == pilot_alias_id("TWulfZ"))
            .unwrap();
        assert_eq!(twulfz.norm_len, 6);
    }

    #[test]
    fn no_match_preselects_nothing() {
        for cfg in [None, Some("SomeoneElse".to_owned()), Some(String::new())] {
            let mut inputs = pilot_like_fixture();
            inputs.cfg_username = cfg.clone();
            let out = select(&inputs, &IdentityParams::default());
            assert!(
                out.iter().all(|s| !s.selected && s.auto_match.is_none()),
                "{cfg:?}"
            );
            let plays: Vec<u32> = out
                .iter()
                .map(|s| {
                    inputs
                        .aliases
                        .iter()
                        .find(|a| a.alias_id == s.alias_id)
                        .unwrap()
                        .n_plays
                })
                .collect();
            assert!(plays.windows(2).all(|w| w[0] >= w[1]), "{plays:?}");
        }
    }

    #[test]
    fn decisions_override_auto() {
        let params = IdentityParams::default();
        let baseline = select(&pilot_like_fixture(), &params);
        let mut inputs = pilot_like_fixture();
        let twulfz = pilot_alias_id("TWulfZ");
        let empty = pilot_alias_id("");
        inputs.decisions.insert(twulfz, Decision::NotMe);
        inputs.decisions.insert(empty, Decision::Me);
        let out = select(&inputs, &params);

        let row =
            |rows: &[AliasSelection], id| rows.iter().find(|s| s.alias_id == id).cloned().unwrap();
        let t = row(&out, twulfz);
        assert!(!t.selected);
        assert_eq!(t.decision, Some(Decision::NotMe));
        assert_eq!(t.auto_match, cfg_match(SessionMatchKind::Prefix));
        let e = row(&out, empty);
        assert!(e.selected);
        assert_eq!(e.decision, Some(Decision::Me));
        assert_eq!(e.auto_match, None);

        let order = |rows: &[AliasSelection]| rows.iter().map(|s| s.alias_id).collect::<Vec<_>>();
        assert_eq!(
            order(&out),
            order(&baseline),
            "decisions never reorder rows"
        );
        for (after, before) in out.iter().zip(&baseline) {
            if after.alias_id != twulfz && after.alias_id != empty {
                assert_eq!(after, before);
            }
        }
    }

    #[test]
    fn linked_account_matches_after_cfg() {
        let params = IdentityParams::default();
        let mut inputs = pilot_like_fixture();
        inputs.cfg_username = None;
        inputs.linked_username = Some("TWulfZ".to_owned());
        let out = select(&inputs, &params);
        let selected: Vec<AliasId> = out
            .iter()
            .filter(|s| s.selected)
            .map(|s| s.alias_id)
            .collect();
        assert_eq!(selected, vec![pilot_alias_id("TWulfZ")]);
        assert_eq!(
            out[0].auto_match,
            Some(AutoMatch {
                source: MatchSource::LinkedAccount,
                kind: SessionMatchKind::Equal
            })
        );

        inputs.cfg_username = Some("TWulfZ".to_owned());
        let out = select(&inputs, &params);
        assert_eq!(out[0].auto_match, cfg_match(SessionMatchKind::Equal));
    }

    #[test]
    fn selection_ids_are_stable() {
        let decisions: Vec<&str> = Decision::ALL.iter().map(|d| d.as_str()).collect();
        let sources: Vec<&str> = MatchSource::ALL.iter().map(|s| s.as_str()).collect();
        let kinds = [
            SessionMatchKind::Equal.as_str(),
            SessionMatchKind::Prefix.as_str(),
        ];
        let text = format!(
            "version: {SELECTION_VERSION}\ndecision: {}\nmatch_source: {}\nsession_match_kind: {}",
            decisions.join(", "),
            sources.join(", "),
            kinds.join(", ")
        );
        insta::assert_snapshot!("selection_ids", text);
        for d in Decision::ALL {
            assert_eq!(Decision::parse(d.as_str()), Some(*d));
        }
        for s in MatchSource::ALL {
            assert_eq!(MatchSource::parse(s.as_str()), Some(*s));
        }
        assert_eq!(Decision::parse("Me"), None);
    }

    fn arb_aliases() -> impl Strategy<Value = Vec<AliasFacts>> {
        prop::collection::vec((".{0,8}", 0u32..5_000), 0..12).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (name, n_plays))| AliasFacts {
                    alias_id: AliasId(i64::try_from(i).unwrap() + 1),
                    raw_name: name.into_bytes(),
                    n_plays,
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn short_alias_never_auto(alias in ".{0,6}", suffix in ".{0,6}", equal in any::<bool>()) {
            let login = if equal { alias.clone() } else { format!("{alias}{suffix}") };
            let inputs = SelectionInputs {
                aliases: vec![AliasFacts { alias_id: AliasId(1), raw_name: alias.clone().into_bytes(), n_plays: 1 }],
                cfg_username: Some(login.clone()),
                linked_username: Some(login),
                decisions: BTreeMap::new(),
            };
            let params = IdentityParams::default();
            let out = select(&inputs, &params);
            if normalize(&alias).chars().count() < 4 {
                prop_assert_eq!(out[0].auto_match, None);
                prop_assert!(!out[0].selected);
            }
        }

        #[test]
        fn order_independent(
            (aliases, shuffled) in arb_aliases().prop_flat_map(|a| (Just(a.clone()), Just(a).prop_shuffle())),
            cfg in prop::option::of(".{0,10}"),
            me_mask in any::<u16>(),
        ) {
            let decisions: BTreeMap<AliasId, Decision> = aliases
                .iter()
                .enumerate()
                .filter(|(i, _)| me_mask & (1 << i) != 0)
                .map(|(i, a)| (a.alias_id, if i % 2 == 0 { Decision::Me } else { Decision::NotMe }))
                .collect();
            let params = IdentityParams::default();
            let a = select(&SelectionInputs { aliases, cfg_username: cfg.clone(), linked_username: None, decisions: decisions.clone() }, &params);
            let b = select(&SelectionInputs { aliases: shuffled, cfg_username: cfg, linked_username: None, decisions }, &params);
            prop_assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
    }
}

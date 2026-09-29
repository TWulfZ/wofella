//! Identity scopes: `scope = (alias set, keymode, exclusion policy)`, `scope_hash =
//! blake3(canonical form)` (§5.6, spec 004 R8). F1–F3 key their fold on the hash.

use std::collections::{BTreeMap, BTreeSet};

use wolluf_core::{AliasId, Game, Keymode, ScopeHash};

const SCOPE_TAG: &[u8] = b"wolluf-scope/1\0";
const FIELD_SEPARATOR: u8 = 0;

/// Which plays a scope leaves out (F3 `exclude_play` feedback). A stable string that is part
/// of the hash from day one, so adding exclusions later is a new id, not a silent change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExclusionPolicy(&'static str);

impl ExclusionPolicy {
    pub const STANDARD_V1: Self = Self("standard@1");
    const ALL: &'static [Self] = &[Self::STANDARD_V1];

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.0 == s)
    }
}

stable_enum!(
    /// Whether a multi-alias entry is folded as one scope or one scope per alias.
    MergeMode { Merged => "merged", Separate => "separate" }
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub game: Game,
    /// Raw name bytes, not ids: local ids never enter the hash, so a rebuilt user.db gives
    /// identical hashes (R8).
    pub alias_raw_names: BTreeSet<Vec<u8>>,
    pub keymode: Keymode,
    pub exclusion: ExclusionPolicy,
}

/// `tag, game, 0, keymode (ASCII decimal), 0, policy, 0, alias count (u32 LE)`, then per alias
/// in byte order its length (u32 LE) and bytes. The length prefixes make the form injective.
pub fn canonical_bytes(scope: &Scope) -> Vec<u8> {
    let mut out = Vec::from(SCOPE_TAG);
    out.extend_from_slice(scope.game.as_str().as_bytes());
    out.push(FIELD_SEPARATOR);
    out.extend_from_slice(scope.keymode.columns().to_string().as_bytes());
    out.push(FIELD_SEPARATOR);
    out.extend_from_slice(scope.exclusion.as_str().as_bytes());
    out.push(FIELD_SEPARATOR);
    out.extend_from_slice(&saturating_u32(scope.alias_raw_names.len()).to_le_bytes());
    for name in &scope.alias_raw_names {
        out.extend_from_slice(&saturating_u32(name.len()).to_le_bytes());
        out.extend_from_slice(name);
    }
    out
}

pub fn scope_hash(scope: &Scope) -> ScopeHash {
    ScopeHash(*blake3::hash(&canonical_bytes(scope)).as_bytes())
}

/// Only inputs above 4 GiB could saturate, far beyond any osu! name or alias count.
fn saturating_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeAlias {
    pub alias_id: AliasId,
    pub raw_name: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeEntry {
    Profile {
        label: String,
        merge_mode: MergeMode,
        aliases: Vec<ScopeAlias>,
    },
    /// Virtual "mixed, not a person" entry: never persisted, its aliases are every alias.
    AllPlayers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedScope {
    pub hash: ScopeHash,
    /// In raw-name byte order, the order the hash sees them.
    pub alias_ids: Vec<AliasId>,
    pub keymode: Keymode,
    /// i18n args for the scope's label: `label` for a profile, `alias` for a separate scope.
    pub label_args: BTreeMap<String, String>,
}

/// `merge` is the URL override; All players has no persisted mode, so it defaults to merged.
/// An entry without aliases resolves to no scope at all.
pub fn resolve(
    entry: &ScopeEntry,
    all_aliases: &[ScopeAlias],
    keymode: Keymode,
    merge: Option<MergeMode>,
) -> Vec<ResolvedScope> {
    let (label, persisted, aliases) = match entry {
        ScopeEntry::Profile {
            label,
            merge_mode,
            aliases,
        } => (Some(label.as_str()), *merge_mode, aliases.as_slice()),
        ScopeEntry::AllPlayers => (None, MergeMode::Merged, all_aliases),
    };
    let mut ordered: Vec<&ScopeAlias> = aliases.iter().collect();
    ordered.sort_by(|a, b| (&a.raw_name, a.alias_id).cmp(&(&b.raw_name, b.alias_id)));
    if ordered.is_empty() {
        return Vec::new();
    }

    let base_args: BTreeMap<String, String> = label
        .map(|l| ("label".to_owned(), l.to_owned()))
        .into_iter()
        .collect();
    let build = |group: &[&ScopeAlias], args: BTreeMap<String, String>| ResolvedScope {
        hash: scope_hash(&Scope {
            // F0 reads one source; F4+ sources carry their own game on the alias.
            game: Game::OsuStable,
            alias_raw_names: group.iter().map(|a| a.raw_name.clone()).collect(),
            keymode,
            exclusion: ExclusionPolicy::STANDARD_V1,
        }),
        alias_ids: group.iter().map(|a| a.alias_id).collect(),
        keymode,
        label_args: args,
    };

    match merge.unwrap_or(persisted) {
        MergeMode::Merged => vec![build(&ordered, base_args)],
        MergeMode::Separate => ordered
            .iter()
            .map(|a| {
                let mut args = base_args.clone();
                args.insert(
                    "alias".to_owned(),
                    String::from_utf8_lossy(&a.raw_name).into_owned(),
                );
                build(std::slice::from_ref(a), args)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Computed once from the canonical form with an independent Python blake3 script, then
    // frozen: a change here orphans every fold row keyed by a scope hash.
    const PILOT_SELF_K7_BYTES: &str = "776f6c6c75662d73636f70652f31006f73755f737461626c650037007374616e646172644031000300000000000000060000005457756c665a170000005457756c665a6173646173646173642064206a53537c7c";
    const PILOT_SELF_K7_HASH: &str =
        "0bcdcc78196b416b672c21835edaa3b4e082c69c72e95a031fb0d0bdd5930873";

    fn names(list: &[&str]) -> BTreeSet<Vec<u8>> {
        list.iter().map(|n| n.as_bytes().to_vec()).collect()
    }

    fn scope(list: &[&str], keymode: Keymode) -> Scope {
        Scope {
            game: Game::OsuStable,
            alias_raw_names: names(list),
            keymode,
            exclusion: ExclusionPolicy::STANDARD_V1,
        }
    }

    fn alias(id: i64, name: &str) -> ScopeAlias {
        ScopeAlias {
            alias_id: AliasId(id),
            raw_name: name.as_bytes().to_vec(),
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn canonical_form_golden() {
        let pilot = scope(&["TWulfZ", "", "TWulfZasdasdasd d jSS||"], Keymode::K7);
        let bytes = canonical_bytes(&pilot);
        let hash = scope_hash(&pilot);
        assert_eq!(hex(&bytes), PILOT_SELF_K7_BYTES);
        assert_eq!(hash.to_string(), PILOT_SELF_K7_HASH);
        insta::assert_snapshot!(
            "scope_canonical_form",
            format!("bytes: {}\nhash: {hash}", hex(&bytes))
        );
    }

    #[test]
    fn hash_invariants() {
        let base = scope(&["TWulfZ", "", "W"], Keymode::K7);
        let reordered = scope(&["W", "TWulfZ", ""], Keymode::K7);
        assert_eq!(scope_hash(&base), scope_hash(&reordered));

        let variants = [
            scope(&["TWulfZ", "", "w"], Keymode::K7),
            scope(&["TWulfZ", ""], Keymode::K7),
            scope(&["TWulfZ", "", "W", "s"], Keymode::K7),
            scope(&["TWulfZ", "", "W"], Keymode::K4),
            Scope {
                exclusion: ExclusionPolicy("test@1"),
                ..base.clone()
            },
        ];
        for variant in &variants {
            assert_ne!(scope_hash(variant), scope_hash(&base), "{variant:?}");
        }
        // Length prefixes keep ("ab", "c") and ("a", "bc") apart.
        assert_ne!(
            scope_hash(&scope(&["ab", "c"], Keymode::K7)),
            scope_hash(&scope(&["a", "bc"], Keymode::K7))
        );

        // Local row ids never enter the hash: a rebuilt user.db gives the same scopes.
        let entry = |ids: [i64; 2]| ScopeEntry::Profile {
            label: "Me".to_owned(),
            merge_mode: MergeMode::Merged,
            aliases: vec![alias(ids[0], "TWulfZ"), alias(ids[1], "W")],
        };
        let a = resolve(&entry([1, 2]), &[], Keymode::K7, None);
        let b = resolve(&entry([40, 7]), &[], Keymode::K7, None);
        assert_eq!(a[0].hash, b[0].hash);
        assert_ne!(a[0].alias_ids, b[0].alias_ids);
    }

    #[test]
    fn resolve_table() {
        let k7 = Keymode::K7;
        let three = vec![alias(3, "W"), alias(1, "TWulfZ"), alias(2, "")];
        let profile = |merge_mode, aliases: Vec<ScopeAlias>| ScopeEntry::Profile {
            label: "Me".to_owned(),
            merge_mode,
            aliases,
        };

        let merged = resolve(&profile(MergeMode::Merged, three.clone()), &[], k7, None);
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].alias_ids,
            vec![AliasId(2), AliasId(1), AliasId(3)]
        );
        assert_eq!(merged[0].hash, scope_hash(&scope(&["", "TWulfZ", "W"], k7)));
        assert_eq!(merged[0].keymode, k7);
        assert_eq!(
            merged[0].label_args.get("label").map(String::as_str),
            Some("Me")
        );

        let separate = resolve(&profile(MergeMode::Separate, three.clone()), &[], k7, None);
        let ids: Vec<Vec<AliasId>> = separate.iter().map(|s| s.alias_ids.clone()).collect();
        assert_eq!(
            ids,
            vec![vec![AliasId(2)], vec![AliasId(1)], vec![AliasId(3)]]
        );
        assert_eq!(separate[1].hash, scope_hash(&scope(&["TWulfZ"], k7)));
        assert_eq!(
            separate[1].label_args.get("alias").map(String::as_str),
            Some("TWulfZ")
        );

        // The URL override wins over the persisted mode.
        let overridden = resolve(
            &profile(MergeMode::Separate, three.clone()),
            &[],
            k7,
            Some(MergeMode::Merged),
        );
        assert_eq!(overridden.len(), 1);

        let everyone = [alias(9, "Rosalind"), alias(1, "TWulfZ"), alias(2, "")];
        let all = resolve(&ScopeEntry::AllPlayers, &everyone, k7, None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].alias_ids, vec![AliasId(2), AliasId(9), AliasId(1)]);
        assert_eq!(
            all[0].hash,
            scope_hash(&scope(&["", "Rosalind", "TWulfZ"], k7))
        );
        let all_separate = resolve(
            &ScopeEntry::AllPlayers,
            &everyone,
            k7,
            Some(MergeMode::Separate),
        );
        assert_eq!(all_separate.len(), 3);

        assert!(resolve(&profile(MergeMode::Merged, vec![]), &[], k7, None).is_empty());
        assert!(resolve(&profile(MergeMode::Separate, vec![]), &[], k7, None).is_empty());
    }

    #[test]
    fn stable_strings() {
        assert_eq!(ExclusionPolicy::STANDARD_V1.as_str(), "standard@1");
        assert_eq!(
            ExclusionPolicy::parse("standard@1"),
            Some(ExclusionPolicy::STANDARD_V1)
        );
        assert_eq!(ExclusionPolicy::parse("standard@2"), None);
        for mode in MergeMode::ALL {
            assert_eq!(MergeMode::parse(mode.as_str()), Some(*mode));
        }
        assert_eq!(MergeMode::Merged.as_str(), "merged");
        assert_eq!(MergeMode::Separate.as_str(), "separate");
    }
}

//! Synthetic pilot-shaped identity fixture (spec 004 AC3). Third-party names are anonymized
//! (docs/conventions.md, fixtures); the R2 play counts are kept so ordering is realistic.

use std::collections::BTreeMap;

use wolluf_core::AliasId;

use super::selection::{AliasFacts, SelectionInputs};

pub(crate) const PILOT_CFG_USERNAME: &str = "TWulfZasdasdasd d jSS||";

/// (raw name, plays) in R2 order; alias ids are 1-based positions in this table.
pub(crate) const PILOT_ALIASES: [(&str, u32); 10] = [
    ("TWulfZ", 3_019),
    ("", 1_395),
    ("W", 344),
    ("Rosalind", 68),
    ("s", 62),
    ("w", 33),
    ("Kovacs", 30),
    ("TWulfZasdasdasd d jSS||", 27),
    ("Wulf", 10),
    ("Sterling", 1),
];

pub(crate) fn pilot_alias_id(raw_name: &str) -> AliasId {
    let pos = PILOT_ALIASES
        .iter()
        .position(|(name, _)| *name == raw_name)
        .unwrap_or_else(|| panic!("{raw_name:?} is not a pilot alias"));
    AliasId(i64::try_from(pos).unwrap() + 1)
}

pub(crate) fn pilot_like_fixture() -> SelectionInputs {
    SelectionInputs {
        aliases: PILOT_ALIASES
            .iter()
            .map(|(name, n_plays)| AliasFacts {
                alias_id: pilot_alias_id(name),
                raw_name: name.as_bytes().to_vec(),
                n_plays: *n_plays,
            })
            .collect(),
        cfg_username: Some(PILOT_CFG_USERNAME.to_owned()),
        linked_username: None,
        decisions: BTreeMap::new(),
    }
}

/// R2 play counts scaled down with their order kept: the service tests assert selection and
/// order, never counts (spec 004 R2).
const PILOT_SCALED_PLAYS: [u32; 10] = [30, 14, 9, 7, 6, 5, 4, 3, 2, 1];
pub(crate) const PILOT_CHART_7K_A: &[u8] = b"pilot 7K chart a";
pub(crate) const PILOT_CHART_7K_B: &[u8] = b"pilot 7K chart b";
/// Played but absent from osu!.db, so its plays land in the `unknown` bucket.
pub(crate) const PILOT_CHART_UNKNOWN: &[u8] = b"pilot chart not in osu!.db";
const PILOT_CFG_ACCOUNT: &str = "fixture";

pub(crate) fn pilot_cfg(username: &str) -> Vec<u8> {
    format!("# fake cfg\r\nUsername = {username}\r\nPassword = WOLLUF_SENTINEL_9f3a\r\n")
        .into_bytes()
}

pub(crate) fn pilot_cfg_path() -> String {
    format!("osu!.{PILOT_CFG_ACCOUNT}.cfg")
}

/// The pilot's scores for `extra` additional `TWulfZ` plays too; `nth` stays unique per play
/// so every natural key differs.
pub(crate) fn pilot_scores(extra_twulfz: u32) -> Vec<wolluf_source_osu::testkit::ScoreBuilder> {
    use crate::features::plays::testkit::md5_hex;
    let charts = [
        md5_hex(PILOT_CHART_7K_A),
        md5_hex(PILOT_CHART_7K_B),
        md5_hex(PILOT_CHART_UNKNOWN),
    ];
    let mut nth = 0_i64;
    let mut out = Vec::new();
    for ((name, _), n) in PILOT_ALIASES.iter().zip(PILOT_SCALED_PLAYS) {
        let n = if *name == "TWulfZ" {
            n + extra_twulfz
        } else {
            n
        };
        for i in 0..n {
            nth += 1;
            let chart = &charts[usize::try_from(i).unwrap() % charts.len()];
            out.push(wolluf_source_osu::testkit::ScoreBuilder::mania(
                chart, name, nth,
            ));
        }
    }
    out
}

/// A synthetic install shaped like the pilot's: the ten R2 aliases (third parties
/// anonymized), two 7K charts in osu!.db and one uncatalogued chart. `cfg_username = None`
/// leaves no account cfg at all.
pub(crate) fn pilot_install(cfg_username: Option<&str>) -> wolluf_source_osu::testkit::FakeInstall {
    use crate::features::plays::testkit::{md5_hex, scores_db};
    use wolluf_source_osu::testkit::{BeatmapBuilder, FakeInstall, OsuDbBuilder};
    let osu_db = OsuDbBuilder::new()
        .beatmap(BeatmapBuilder::mania(&md5_hex(PILOT_CHART_7K_A), 7).build())
        .beatmap(BeatmapBuilder::mania(&md5_hex(PILOT_CHART_7K_B), 7).build())
        .encode();
    let install = FakeInstall::new()
        .without(pilot_cfg_path())
        .osu_db(osu_db)
        .scores_db(scores_db(&pilot_scores(0)));
    match cfg_username {
        Some(name) => install.cfg(PILOT_CFG_ACCOUNT, pilot_cfg(name)),
        None => install,
    }
}

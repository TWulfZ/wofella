//! One ledger play from a score body. scores.db records and `.osr` headers share the layout
//! (002's `ScoreHeader`), so ingest and orphan import build plays the same way (spec 003).

use std::str::FromStr;

use wolluf_core::{
    AliasId, BlobSha256, ChartMd5, DotNetTicks, ErrorCode, FileTime, Game, PlayId, UnixUs,
};
use wolluf_source_osu::codec::score_header::{JudgementCounts, OnlineId, ScoreHeader};
use wolluf_store::repo::ledger::{NewPlay, PlayCounts, PlayOrigin, ScoreSystem, SnapshotId};

use crate::jobs::ItemError;

/// Judgement weights of the display accuracy (§5.1), from the `osudb.py` oracle
/// (`acc_v1`, `acc_max`). Display only: nothing derived reads `native_acc`.
struct AccWeights {
    max: u32,
    n300: u32,
    n200: u32,
    n100: u32,
    n50: u32,
    /// The weight of a perfect judgement, which the denominator scales by.
    per_judgement: u32,
}

const ACC_V1: AccWeights = AccWeights {
    max: 300,
    n300: 300,
    n200: 200,
    n100: 100,
    n50: 50,
    per_judgement: 300,
};

const ACC_V2: AccWeights = AccWeights {
    max: 305,
    n300: 300,
    n200: 200,
    n100: 100,
    n50: 50,
    per_judgement: 305,
};

/// In mania `geki` is MAX and `katu` is 200. `None` when there are no judgements.
pub(crate) fn native_acc(counts: &JudgementCounts, score_v2: bool) -> Option<f64> {
    let w = if score_v2 { &ACC_V2 } else { &ACC_V1 };
    let total = counts.total();
    if total == 0 {
        return None;
    }
    let points = w.max * u32::from(counts.geki)
        + w.n300 * u32::from(counts.n300)
        + w.n200 * u32::from(counts.katu)
        + w.n100 * u32::from(counts.n100)
        + w.n50 * u32::from(counts.n50);
    Some(f64::from(points) / (f64::from(w.per_judgement) * f64::from(total)))
}

/// Everything of a play except the ids that only exist inside a write transaction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlayDraft {
    pub(crate) raw_name: Vec<u8>,
    pub(crate) id: PlayId,
    pub(crate) chart_md5: ChartMd5,
    pub(crate) filetime: FileTime,
    played_at: UnixUs,
    mods: i32,
    score_system: ScoreSystem,
    counts: PlayCounts,
    max_combo: u16,
    score: i32,
    native_acc: Option<f64>,
    online_score_id: Option<u64>,
    client_version: i32,
}

pub(crate) fn chart_md5(header: &ScoreHeader) -> Result<ChartMd5, ItemError> {
    let invalid = || ItemError::new(ErrorCode::InvalidInput, "beatmap md5 is not lowercase hex");
    let text = header
        .beatmap_md5
        .as_bytes()
        .and_then(|b| std::str::from_utf8(b).ok())
        .ok_or_else(invalid)?;
    ChartMd5::from_str(text).map_err(|_| invalid())
}

pub(crate) fn draft(header: &ScoreHeader, online_id: OnlineId) -> Result<PlayDraft, ItemError> {
    let chart_md5 = chart_md5(header)?;
    let ticks = DotNetTicks(header.timestamp_ticks);
    let filetime = ticks.to_filetime().ok_or_else(|| {
        ItemError::new(
            ErrorCode::InvalidInput,
            "timestamp is before the FILETIME epoch",
        )
    })?;
    let raw_name = header.player.bytes_or_empty().to_vec();
    let c = header.counts;
    Ok(PlayDraft {
        id: PlayId::derive(Game::OsuStable, chart_md5, &raw_name, filetime),
        raw_name,
        chart_md5,
        filetime,
        // scores.db ticks are UTC (ADR 0014), so no timezone conversion.
        played_at: ticks.to_unix_us(),
        // Bit pattern kept: mod flags are a mask, not a number.
        mods: i32::from_ne_bytes(header.mods.to_ne_bytes()),
        score_system: if header.is_score_v2() {
            ScoreSystem::V2
        } else {
            ScoreSystem::V1
        },
        counts: PlayCounts {
            max: c.geki,
            n300: c.n300,
            n200: c.katu,
            n100: c.n100,
            n50: c.n50,
            miss: c.miss,
        },
        max_combo: header.max_combo,
        score: header.score,
        native_acc: native_acc(&c, header.is_score_v2()),
        online_score_id: online_id.positive(),
        client_version: header.version,
    })
}

pub(crate) struct Links {
    pub(crate) origin: PlayOrigin,
    pub(crate) snapshot_id: Option<SnapshotId>,
    pub(crate) replay_sha: Option<BlobSha256>,
    pub(crate) osg_sha: Option<BlobSha256>,
}

impl PlayDraft {
    pub(crate) fn into_new_play(self, alias_id: AliasId, links: Links, now: UnixUs) -> NewPlay {
        NewPlay {
            id: self.id,
            alias_id,
            chart_md5: self.chart_md5,
            origin: links.origin,
            filetime: self.filetime,
            played_at: self.played_at,
            mods: self.mods,
            score_system: self.score_system,
            counts: self.counts,
            max_combo: self.max_combo,
            score: self.score,
            native_acc: self.native_acc,
            online_score_id: self.online_score_id,
            client_version: self.client_version,
            replay_sha: links.replay_sha,
            osg_sha: links.osg_sha,
            snapshot_id: links.snapshot_id,
            ingested_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use wolluf_source_osu::codec::score_header::mods;
    use wolluf_source_osu::testkit::ScoreBuilder;

    use super::*;

    const EPS: f64 = 1e-12;

    #[test]
    fn native_acc_v1_v2_matches_oracle() {
        let counts = JudgementCounts {
            geki: 100,
            n300: 50,
            katu: 10,
            n100: 5,
            n50: 2,
            miss: 3,
        };
        // research/scripts/audit/osudb.py acc_v1 / acc_max over the same counts.
        assert!((native_acc(&counts, false).unwrap() - 0.933_333_333_333_333_3).abs() < EPS);
        assert!((native_acc(&counts, true).unwrap() - 0.927_675_988_428_158_1).abs() < EPS);
        let misses = JudgementCounts {
            miss: 7,
            ..JudgementCounts::default()
        };
        assert_eq!(native_acc(&misses, false), Some(0.0));
        assert_eq!(native_acc(&JudgementCounts::default(), true), None);
    }

    #[test]
    fn draft_maps_header_fields() {
        let md5 = "e956977ccc1d74a50ae48b43a868cc20";
        let header = ScoreBuilder::mania(md5, "TWulfZ", 3)
            .mods(mods::SCORE_V2 | 1 << 31)
            .build_header();
        let d = draft(&header, OnlineId::I64(-4)).unwrap();
        let filetime = DotNetTicks(header.timestamp_ticks).to_filetime().unwrap();
        assert_eq!(
            d.id,
            PlayId::derive(Game::OsuStable, md5.parse().unwrap(), b"TWulfZ", filetime)
        );
        assert_eq!(d.score_system, ScoreSystem::V2);
        assert!(d.mods < 0, "the high bit survives as a mask");
        assert_eq!(d.online_score_id, None, "ids ≤ 0 mean no online score");
        assert_eq!(d.counts.max, header.counts.geki);
        assert_eq!(
            d.played_at,
            DotNetTicks(header.timestamp_ticks).to_unix_us()
        );
    }

    #[test]
    fn draft_rejects_bad_md5_and_pre_1601_ticks() {
        let upper = ScoreBuilder::mania("E956977CCC1D74A50AE48B43A868CC20", "x", 1).build_header();
        assert_eq!(
            draft(&upper, OnlineId::Absent).unwrap_err().code,
            ErrorCode::InvalidInput
        );
        let early = ScoreBuilder::mania("e956977ccc1d74a50ae48b43a868cc20", "x", 1)
            .ticks(DotNetTicks(1))
            .build_header();
        assert_eq!(
            draft(&early, OnlineId::Absent).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

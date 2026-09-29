//! The score body shared by scores.db records and the `.osr` header (`legacy_db.md`: "apart
//! from the online score ID, the individual score format is the same as the replay format").

use crate::codec::version::{OnlineIdWidth, online_id_width};
use crate::codec::{OsuString, Reader, Writer};
use crate::error::CodecError;

/// Mod bits that change the layout or the judging model (`osr_wiki.md`, "Mods").
pub mod mods {
    pub const TARGET_PRACTICE: u32 = 1 << 23;
    pub const SCORE_V2: u32 = 1 << 29;
}

/// Judgement counts in on-disk order; in mania `geki` is MAX and `katu` is 200.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct JudgementCounts {
    pub n300: u16,
    pub n100: u16,
    pub n50: u16,
    pub geki: u16,
    pub katu: u16,
    pub miss: u16,
}

impl JudgementCounts {
    pub fn total(&self) -> u32 {
        [
            self.n300, self.n100, self.n50, self.geki, self.katu, self.miss,
        ]
        .into_iter()
        .map(u32::from)
        .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreHeader {
    pub mode: u8,
    pub version: i32,
    pub beatmap_md5: OsuString,
    pub player: OsuString,
    pub replay_md5: OsuString,
    pub counts: JudgementCounts,
    pub score: i32,
    pub max_combo: u16,
    /// Raw byte: osu! treats any non-zero value as true.
    pub perfect: u8,
    pub mods: u32,
    pub life_bar: OsuString,
    /// Raw .NET ticks; convert with `wolluf_core::DotNetTicks`.
    pub timestamp_ticks: i64,
}

impl ScoreHeader {
    pub const fn is_score_v2(&self) -> bool {
        self.mods & mods::SCORE_V2 != 0
    }

    pub const fn has_target_practice(&self) -> bool {
        self.mods & mods::TARGET_PRACTICE != 0
    }
}

/// Width follows the record's own version (lazer `LegacyScoreDecoder`); keeping the variant
/// makes encode the exact inverse of decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnlineId {
    Absent,
    I32(i32),
    I64(i64),
}

impl OnlineId {
    pub const fn for_version(version: i32, value: i64) -> Self {
        match online_id_width(version) {
            OnlineIdWidth::Absent => Self::Absent,
            OnlineIdWidth::I32 => Self::I32(value as i32),
            OnlineIdWidth::I64 => Self::I64(value),
        }
    }

    /// Local and unsubmitted scores store 0 (4,718 of 4,989 on the pilot); ≤ 0 means "none".
    pub fn positive(self) -> Option<u64> {
        let value = match self {
            Self::Absent => return None,
            Self::I32(v) => i64::from(v),
            Self::I64(v) => v,
        };
        u64::try_from(value).ok().filter(|&v| v > 0)
    }
}

pub fn read_score_header(r: &mut Reader<'_>) -> Result<ScoreHeader, CodecError> {
    Ok(ScoreHeader {
        mode: r.u8()?,
        version: r.i32()?,
        beatmap_md5: r.osu_string()?,
        player: r.osu_string()?,
        replay_md5: r.osu_string()?,
        counts: JudgementCounts {
            n300: r.u16()?,
            n100: r.u16()?,
            n50: r.u16()?,
            geki: r.u16()?,
            katu: r.u16()?,
            miss: r.u16()?,
        },
        score: r.i32()?,
        max_combo: r.u16()?,
        perfect: r.u8()?,
        mods: r.u32()?,
        life_bar: r.osu_string()?,
        timestamp_ticks: r.i64()?,
    })
}

pub fn write_score_header(w: &mut Writer, h: &ScoreHeader) {
    w.u8(h.mode);
    w.i32(h.version);
    w.osu_string(&h.beatmap_md5);
    w.osu_string(&h.player);
    w.osu_string(&h.replay_md5);
    let c = &h.counts;
    for n in [c.n300, c.n100, c.n50, c.geki, c.katu, c.miss] {
        w.u16(n);
    }
    w.i32(h.score);
    w.u16(h.max_combo);
    w.u8(h.perfect);
    w.u32(h.mods);
    w.osu_string(&h.life_bar);
    w.i64(h.timestamp_ticks);
}

pub fn read_online_id(r: &mut Reader<'_>, version: i32) -> Result<OnlineId, CodecError> {
    Ok(match online_id_width(version) {
        OnlineIdWidth::Absent => OnlineId::Absent,
        OnlineIdWidth::I32 => OnlineId::I32(r.i32()?),
        OnlineIdWidth::I64 => OnlineId::I64(r.i64()?),
    })
}

pub fn write_online_id(w: &mut Writer, id: OnlineId) {
    match id {
        OnlineId::Absent => {}
        OnlineId::I32(v) => w.i32(v),
        OnlineId::I64(v) => w.i64(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{FileKind, Reader, Writer};

    #[test]
    fn header_roundtrip_and_counts_total() {
        let header = ScoreHeader {
            mode: 3,
            version: 20_260_924,
            beatmap_md5: OsuString::present(*b"0123456789abcdef0123456789abcdef"),
            player: OsuString::present(*b"W"),
            replay_md5: OsuString::Absent,
            counts: JudgementCounts {
                n300: 1,
                n100: 2,
                n50: 3,
                geki: 4,
                katu: 5,
                miss: 6,
            },
            score: 123,
            max_combo: 7,
            perfect: 0,
            mods: mods::SCORE_V2,
            life_bar: OsuString::present(*b""),
            timestamp_ticks: 639_190_703_004_225_018,
        };
        let mut w = Writer::new();
        write_score_header(&mut w, &header);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes, FileKind::Osr);
        assert_eq!(read_score_header(&mut r), Ok(header.clone()));
        assert_eq!(r.remaining(), 0);
        assert_eq!(header.counts.total(), 21);
        assert!(header.is_score_v2());
        assert!(!header.has_target_practice());
    }

    #[test]
    fn online_id_positive_treats_non_positive_as_none() {
        assert_eq!(OnlineId::I64(0).positive(), None);
        assert_eq!(OnlineId::I64(-5).positive(), None);
        assert_eq!(OnlineId::I32(7).positive(), Some(7));
        assert_eq!(OnlineId::I64(1 << 40).positive(), Some(1 << 40));
        assert_eq!(OnlineId::Absent.positive(), None);
        assert_eq!(OnlineId::for_version(20_220_424, 9), OnlineId::I64(9));
        assert_eq!(OnlineId::for_version(20_130_101, 9), OnlineId::I32(9));
        assert_eq!(OnlineId::for_version(20_100_101, 9), OnlineId::Absent);
    }
}

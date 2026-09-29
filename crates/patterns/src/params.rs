//! Every threshold the pattern engine reads (D17). Values are integers in µs, beat ticks
//! ([`TICKS_PER_BEAT`]), row counts or permille, so the canonical hash never depends on float
//! formatting. Defaults cite Interlude prelude `Patterns.fs` (MIT, see NOTICE) where a rule has
//! an equivalent there; the rest are uncalibrated placeholders, tuned later on the dev split and
//! never on the gold set.

use serde::Serialize;

/// Beat subdivision unit for gaps. 192 is the LCM of 64 and 48, so every power-of-two snap up
/// to 1/64 and every triplet snap up to 1/48 is a whole number of ticks.
pub const TICKS_PER_BEAT: u32 = 192;

const PARAMS_TAG: &[u8] = b"wolluf.patterns.params.v1";

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PatternParams {
    pub view: ViewParams,
    pub jack: JackParams,
    pub stream: StreamParams,
    pub speed: SpeedParams,
    pub tech: TechParams,
    pub ln: LnParams,
    pub segment: SegmentParams,
}

impl PatternParams {
    /// blake3 over a domain tag and the postcard encoding, which follows field declaration
    /// order; it goes into the patterns stage vkey.
    pub fn params_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PARAMS_TAG);
        // Derived `Serialize` over integers and `Vec`s cannot fail with the allocating flavour;
        // the error arm still hashes to a distinct value instead of panicking.
        match postcard::to_allocvec(self) {
            Ok(bytes) => hasher.update(&bytes),
            Err(err) => hasher.update(err.to_string().as_bytes()),
        };
        *hasher.finalize().as_bytes()
    }
}

/// Inputs of the [`crate::ChartView`] primitives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViewParams {
    /// Width of the window centred on each row for `RowFeat::density_milli`.
    pub density_window_us: i64,
    /// Ascending. A gap snaps to the first divisor `d` with a whole number of `1/d` beats.
    pub snap_divisors: Vec<u8>,
    pub snap_tolerance_us: i64,
}

impl Default for ViewParams {
    fn default() -> Self {
        Self {
            density_window_us: 1_000_000,
            // The osu! stable editor's beat-snap divisors.
            snap_divisors: vec![1, 2, 3, 4, 6, 8, 12, 16],
            // Stable stores integer milliseconds, so each end of a gap can be off by up to 1 ms.
            snap_tolerance_us: 2_000,
        }
    }
}

/// Jack lengths are not here: minijack = 2 and longjack = 3+ are vocabulary (ADR 0017).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JackParams {
    /// Interlude `Core.JACKS` requires `4 × gap < 2000 ms`.
    pub max_gap_us: i64,
    pub max_gap_ticks: u32,
    /// Interlude `CHORDJACKS` wants a chord of 3+ followed by one of 2+.
    pub chordjack_min_first_notes: u32,
    pub chordjack_min_next_notes: u32,
    pub chordjack_min_jacks: u32,
    /// "Every other row" in the taxonomy is a period of 2.
    pub anchor_max_period_rows: u32,
    pub anchor_min_hits: u32,
}

impl Default for JackParams {
    fn default() -> Self {
        Self {
            max_gap_us: 500_000,
            max_gap_ticks: TICKS_PER_BEAT,
            chordjack_min_first_notes: 3,
            chordjack_min_next_notes: 2,
            chordjack_min_jacks: 1,
            anchor_max_period_rows: 2,
            anchor_min_hits: 4,
        }
    }
}

/// Chord sizes that define stream leaves (jump = 2, hand = 3, dense = 4+, jumptrill chords > 4)
/// are vocabulary (ADR 0017, taxonomy) and stay out of here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StreamParams {
    /// Interlude `Core.STREAM`: 5 single-note rows without jacks.
    pub min_rows: u32,
    pub max_gap_ticks: u32,
    /// Interlude `Core.CHORDSTREAM`: 4 jackless rows.
    pub chordstream_min_rows: u32,
    /// Interlude `Stream_4K.ROLL`: 3 rows in one direction.
    pub roll_min_rows: u32,
    /// Interlude `Stream_4K.TRILL`: 4 rows, `a b a b`.
    pub trill_min_rows: u32,
    /// Interlude `Chordstream_4K.JUMPTRILL`: 4 two-note rows.
    pub jumptrill_min_rows: u32,
    /// Interlude `Chordstream_4K.SPLITTRILL`: 3 two-note rows.
    pub split_trill_min_rows: u32,
    pub bracket_min_rows: u32,
    /// Interlude `Chordstream_7K.BRACKETS`: 3 jackless, non-roll chord rows.
    pub chordbracket_min_rows: u32,
}

impl Default for StreamParams {
    fn default() -> Self {
        Self {
            min_rows: 5,
            max_gap_ticks: TICKS_PER_BEAT / 4,
            chordstream_min_rows: 4,
            roll_min_rows: 3,
            trill_min_rows: 4,
            jumptrill_min_rows: 4,
            split_trill_min_rows: 3,
            bracket_min_rows: 4,
            chordbracket_min_rows: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeedParams {
    pub burst_max_rows: u32,
    /// Surrounding window the burst's density is compared against.
    pub burst_context_us: i64,
    pub burst_min_density_ratio_permille: u32,
}

impl Default for SpeedParams {
    fn default() -> Self {
        Self {
            burst_max_rows: 12,
            burst_context_us: 4_000_000,
            burst_min_density_ratio_permille: 1_500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TechParams {
    /// Sliding window, in press rows, for the share-based tech rules.
    pub window_rows: u32,
    pub irregular_min_offsnap_permille: u32,
    pub hand_imbalance_min_share_permille: u32,
    pub thumb_min_share_permille: u32,
}

impl Default for TechParams {
    fn default() -> Self {
        Self {
            window_rows: 16,
            irregular_min_offsnap_permille: 250,
            hand_imbalance_min_share_permille: 700,
            thumb_min_share_permille: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LnParams {
    /// Shorter LNs play like taps and do not count as LNs.
    pub min_len_ticks: u32,
    pub density_max_gap_ticks: u32,
    pub chord_min_notes: u32,
    pub hybrid_min_taps: u32,
    /// Tap then LN head on the same column.
    pub shield_max_gap_ticks: u32,
    /// Tail to the next head on the same column.
    pub inverse_max_gap_ticks: u32,
    pub release_min_stagger_ticks: u32,
}

impl Default for LnParams {
    fn default() -> Self {
        Self {
            min_len_ticks: TICKS_PER_BEAT / 8,
            density_max_gap_ticks: TICKS_PER_BEAT / 2,
            chord_min_notes: 2,
            hybrid_min_taps: 2,
            shield_max_gap_ticks: TICKS_PER_BEAT / 4,
            inverse_max_gap_ticks: TICKS_PER_BEAT / 4,
            release_min_stagger_ticks: TICKS_PER_BEAT / 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SegmentParams {
    pub min_rows: u32,
    pub min_len_us: i64,
    pub max_len_us: i64,
    /// Same-pattern candidates closer than this merge into one segment.
    pub merge_gap_ticks: u32,
    /// Share of a segment's rows the primary pattern must cover.
    pub min_purity_permille: u32,
    /// Share a pattern needs to be listed as a secondary tag.
    pub secondary_min_share_permille: u32,
}

impl Default for SegmentParams {
    fn default() -> Self {
        Self {
            min_rows: 4,
            min_len_us: 500_000,
            max_len_us: 16_000_000,
            merge_gap_ticks: TICKS_PER_BEAT / 2,
            min_purity_permille: 600,
            secondary_min_share_permille: 200,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: [u8; 32]) -> String {
        blake3::Hash::from(bytes).to_hex().to_string()
    }

    // Frozen on first computation: any change to a default or to the params layout moves every
    // pattern vkey, so it must be deliberate.
    const DEFAULT_HASH: &str = "0d79a30d1f6ada4d6ed5c0446241f29661d9d74b7a732ce2bcb45413e6872ea2";

    #[test]
    fn default_params_hash_is_frozen() {
        assert_eq!(hex(PatternParams::default().params_hash()), DEFAULT_HASH);
    }

    #[test]
    fn hash_is_deterministic() {
        let params = PatternParams::default();
        assert_eq!(params.params_hash(), params.clone().params_hash());
    }

    #[test]
    fn every_family_moves_the_hash() {
        let base = PatternParams::default().params_hash();
        let edits: [fn(&mut PatternParams); 7] = [
            |p| p.view.density_window_us += 1,
            |p| p.jack.max_gap_ticks += 1,
            |p| p.stream.min_rows += 1,
            |p| p.speed.burst_max_rows += 1,
            |p| p.tech.window_rows += 1,
            |p| p.ln.min_len_ticks += 1,
            |p| p.segment.min_rows += 1,
        ];
        let mut seen = vec![base];
        for edit in edits {
            let mut params = PatternParams::default();
            edit(&mut params);
            let hash = params.params_hash();
            assert!(!seen.contains(&hash));
            seen.push(hash);
        }
    }

    #[test]
    fn snap_divisors_ascend_from_one() {
        let divisors = PatternParams::default().view.snap_divisors;
        assert_eq!(divisors.first(), Some(&1));
        assert!(divisors.windows(2).all(|w| w[0] < w[1]));
    }
}

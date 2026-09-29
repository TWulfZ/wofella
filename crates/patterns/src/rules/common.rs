//! Helpers shared by the jack rules.

use wolluf_core::{ColMask, Keymode};

use crate::params::JackParams;
use crate::rule::STRENGTH_MAX;
use crate::view::{RowFeat, ticks};

/// A gap is jack-fast when it is within both the absolute cap and, under a red line, the
/// beat-relative one.
pub(super) fn jack_gap_ok(gap_us: i64, beat_us: Option<i64>, params: &JackParams) -> bool {
    gap_us <= params.max_gap_us
        && beat_us.is_none_or(|beat| ticks(gap_us, beat) <= params.max_gap_ticks)
}

/// Indices of the rows that press something; release-only rows never shape a jack.
pub(super) fn press_rows(rows: &[RowFeat]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| !r.press.is_empty())
        .map(|(i, _)| i)
        .collect()
}

pub(super) fn notes_between(rows: &[RowFeat], first: usize, last: usize) -> u64 {
    rows.get(first..=last)
        .map_or(0, |span| span.iter().map(|r| u64::from(r.notes)).sum())
}

/// `STRENGTH_MAX × part / whole`, clamped; 0 for an empty whole.
pub(super) fn permille(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return 0;
    }
    let value = part.saturating_mul(u64::from(STRENGTH_MAX)) / whole;
    u32::try_from(value.min(u64::from(STRENGTH_MAX))).unwrap_or(STRENGTH_MAX)
}

pub(super) fn single(k: Keymode, col: u8) -> ColMask {
    ColMask::single(k, col).unwrap_or_default()
}

/// A maximal run of presses in one column where each press is on the next press row and
/// jack-fast. `len` counts presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ColumnRun {
    pub col: u8,
    pub first: usize,
    pub last: usize,
    pub len: u32,
}

pub(super) fn column_runs(rows: &[RowFeat], k: Keymode, params: &JackParams) -> Vec<ColumnRun> {
    let mut open: Vec<Option<ColumnRun>> = vec![None; usize::from(k.columns())];
    let mut done = Vec::new();
    for i in press_rows(rows) {
        let Some(row) = rows.get(i) else { continue };
        for (col, slot) in (0u8..).zip(open.iter_mut()) {
            if !row.press.contains(col) {
                done.extend(slot.take());
                continue;
            }
            // An open run means the previous press row pressed this column too.
            let fast = row
                .press_gap_us
                .is_some_and(|gap| jack_gap_ok(gap, row.beat_us, params));
            match slot {
                Some(run) if fast => {
                    run.last = i;
                    run.len += 1;
                }
                _ => {
                    done.extend(slot.replace(ColumnRun {
                        col,
                        first: i,
                        last: i,
                        len: 1,
                    }));
                }
            }
        }
    }
    done.extend(open.into_iter().flatten());
    done
}

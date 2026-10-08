//! MinaCalc v527 over a normalized [`Chart`] (ADR 0022). Calculator floats leave this module only
//! as centi-MSD `i32`.

use serde::Serialize;
use wolluf_chart::Chart;
use wolluf_minacalc::{Calc, CalcError, NoteRow, Skillsets};

const PARAMS_TAG: &[u8] = b"wolluf.difficulty.minacalc.params.v1";

const US_PER_SECOND: f64 = 1_000_000.0;
const MILLI_PER_UNIT: f32 = 1_000.0;
const CENTI_PER_UNIT: f32 = 100.0;
const PERMILLE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MinaCalcParams {
    /// Ascending music rates, in thousandths.
    pub rate_grid_milli: Vec<u16>,
    /// MinaCalc reads LN heads as taps and ignores holds and releases, so from this LN share on
    /// its numbers say nothing about the chart.
    pub ln_unrated_hold_share_permille: u16,
}

impl Default for MinaCalcParams {
    fn default() -> Self {
        Self {
            rate_grid_milli: (700..=1500).step_by(50).collect(),
            ln_unrated_hold_share_permille: 400,
        }
    }
}

impl MinaCalcParams {
    /// blake3 over a domain tag and the postcard encoding, which follows field declaration order.
    pub fn params_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PARAMS_TAG);
        // Derived `Serialize` over integers and a `Vec` cannot fail with the allocating flavour;
        // the error arm still hashes to a distinct value instead of panicking.
        match postcard::to_allocvec(self) {
            Ok(bytes) => hasher.update(&bytes),
            Err(err) => hasher.update(err.to_string().as_bytes()),
        };
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteRowsOut {
    pub rows: Vec<NoteRow>,
    /// LN objects over all objects (taps + LNs): the community "LN ratio" by which LN dans and
    /// LN maps are told apart.
    pub hold_share_permille: u16,
}

/// Taps and LN heads per row, in seconds from the first row; tail-only rows are dropped.
pub fn note_rows(chart: &Chart) -> NoteRowsOut {
    let mut rows: Vec<NoteRow> = Vec::with_capacity(chart.rows().len());
    let mut first_us: Option<i64> = None;
    let mut taps: u64 = 0;
    for row in chart.rows() {
        let notes = u32::from(row.tap.bits() | row.ln_head.bits());
        if notes == 0 {
            continue;
        }
        taps += u64::from(row.tap.len());
        let first = *first_us.get_or_insert(row.t.0);
        let time_s = ((row.t.0 - first) as f64 / US_PER_SECOND) as f32;
        match rows.last_mut() {
            // Rows under one f32 step apart (µs-timed charts, very late rows) would break the
            // calc's strictly-increasing contract; they are one chord at that precision.
            Some(last) if time_s <= last.time_s => last.notes |= notes,
            _ => rows.push(NoteRow { notes, time_s }),
        }
    }
    let lns = chart.ln_pairs().len() as u64;
    let hold_share_permille = (lns * PERMILLE)
        .checked_div(taps + lns)
        .map_or(0, |p| u16::try_from(p).unwrap_or(u16::MAX));
    NoteRowsOut {
        rows,
        hold_share_permille,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnratedReason {
    LnHeavy,
    CalcRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsdStatus {
    Rated,
    Unrated(UnratedReason),
}

/// `centi` in `wolluf_minacalc::SKILLSET_IDS` order, MSD × 100 rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsdAtRate {
    pub rate_milli: u16,
    pub centi: [i32; 8],
}

/// `rows` follows the params' rate grid when `Rated` and is empty otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsdTable {
    pub status: MsdStatus,
    pub hold_share_permille: u16,
    pub rows: Vec<MsdAtRate>,
}

pub fn msd_table(calc: &mut Calc, chart: &Chart, params: &MinaCalcParams) -> MsdTable {
    let NoteRowsOut {
        rows: note_rows,
        hold_share_permille,
    } = note_rows(chart);
    let unrated = |reason| MsdTable {
        status: MsdStatus::Unrated(reason),
        hold_share_permille,
        rows: Vec::new(),
    };
    if hold_share_permille >= params.ln_unrated_hold_share_permille {
        return unrated(UnratedReason::LnHeavy);
    }
    let keycount = chart.keymode().columns();
    let mut rows = Vec::with_capacity(params.rate_grid_milli.len());
    for &rate_milli in &params.rate_grid_milli {
        match calc.msd(&note_rows, rate(rate_milli), keycount) {
            Ok(skillsets) => rows.push(MsdAtRate {
                rate_milli,
                centi: centi(skillsets),
            }),
            Err(_) => return unrated(UnratedReason::CalcRejected),
        }
    }
    MsdTable {
        status: MsdStatus::Rated,
        hold_share_permille,
        rows,
    }
}

/// SSR for a score at `goal` (Wife%, 1.0 = 100%), MSD × 100 rounded.
pub fn ssr_centi(
    calc: &mut Calc,
    rows: &[NoteRow],
    rate_milli: u16,
    goal: f32,
    keycount: u8,
) -> Result<[i32; 8], CalcError> {
    calc.ssr(rows, rate(rate_milli), goal, keycount).map(centi)
}

fn rate(rate_milli: u16) -> f32 {
    f32::from(rate_milli) / MILLI_PER_UNIT
}

/// The D3 quantisation of ADR 0022. Rounding absorbs most libm noise, but a value near x.xx5 can still
/// differ by 1 centi across platforms, so centi never enters a cross-platform hash or golden.
fn centi(skillsets: Skillsets) -> [i32; 8] {
    skillsets.0.map(|v| (v * CENTI_PER_UNIT).round() as i32)
}

#[cfg(test)]
mod tests;

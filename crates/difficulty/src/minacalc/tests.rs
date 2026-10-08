use proptest::prelude::*;
use wolluf_chart::{Chart, ChartMeta, Diagnostics, Note, NoteKind, chart};
use wolluf_core::{Keymode, TimeUs};
use wolluf_minacalc::{Calc, CalcError, NoteRow};

use super::*;

fn hex(bytes: [u8; 32]) -> String {
    blake3::Hash::from(bytes).to_hex().to_string()
}

fn build(keymode: Keymode, notes: Vec<Note>) -> Chart {
    let mut diags = Diagnostics::new();
    Chart::from_notes(keymode, ChartMeta::default(), Vec::new(), notes, &mut diags)
}

fn tap_us(t: i64, col: u8) -> Note {
    Note {
        t: TimeUs(t),
        col,
        kind: NoteKind::Tap,
    }
}

fn stream_4k(bpm: i64, seconds: i64) -> Chart {
    stream(Keymode::K4, bpm, seconds)
}

/// Jack-free pseudo-random stream: MinaCalc nerfs pure rolls as they speed up.
fn stream(keymode: Keymode, bpm: i64, seconds: i64) -> Chart {
    let keys = u32::from(keymode.columns());
    let step_us = 60_000_000 / bpm / 4;
    let mut seed: u32 = 12_345;
    let mut last = u8::MAX;
    let mut notes = Vec::new();
    for i in 0..(seconds * 1_000_000 / step_us) {
        let col = loop {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let col = ((seed >> 16) % keys) as u8;
            if col != last {
                break col;
            }
        };
        last = col;
        notes.push(tap_us(i * step_us, col));
    }
    build(keymode, notes)
}

fn calc() -> Calc {
    Calc::new().unwrap()
}

// Frozen on first computation: any change to a default or to the params layout moves every
// difficulty vkey, so it must be deliberate.
const DEFAULT_HASH: &str = "cb990fdd5066e7eb370357aa95801b95f54fefca44655a207b735476ce5570aa";

#[test]
fn default_params() {
    let params = MinaCalcParams::default();
    let expected: Vec<u16> = (0..=16).map(|i| 700 + 50 * i).collect();
    assert_eq!(params.rate_grid_milli, expected);
    assert_eq!(params.rate_grid_milli.last(), Some(&1500));
    assert_eq!(params.ln_unrated_hold_share_permille, 400);
}

#[test]
fn default_params_hash_is_frozen() {
    assert_eq!(hex(MinaCalcParams::default().params_hash()), DEFAULT_HASH);
}

#[test]
fn every_field_moves_the_hash() {
    let base = MinaCalcParams::default();
    let mut grid = base.clone();
    grid.rate_grid_milli.push(1550);
    let mut cut = base.clone();
    cut.ln_unrated_hold_share_permille = 450;
    assert_ne!(grid.params_hash(), base.params_hash());
    assert_ne!(cut.params_hash(), base.params_hash());
}

#[test]
fn rows_take_taps_and_heads_and_ignore_tails() {
    let chart = chart![step = 100;
        "[..x",
        "|x..",
        "]...",
        "x[..",
        ".]x.",
    ];
    let out = note_rows(&chart);
    let masks: Vec<u32> = out.rows.iter().map(|r| r.notes).collect();
    assert_eq!(masks, vec![0b1001, 0b0010, 0b0011, 0b0100]);
}

#[test]
fn times_are_seconds_from_the_first_row_and_strictly_increasing() {
    let chart = chart![step = 250, start = 1000;
        "x...",
        ".x..",
        "....",
        "..x.",
    ];
    let out = note_rows(&chart);
    let times: Vec<f32> = out.rows.iter().map(|r| r.time_s).collect();
    assert_eq!(times, vec![0.0, 0.25, 0.75]);
}

#[test]
fn rows_whose_seconds_collide_in_f32_are_merged() {
    // 0.5 ms apart near 20000 s, where an f32 step is about 2 ms.
    let far = 20_000_000_000;
    let chart = build(
        Keymode::K4,
        vec![tap_us(0, 0), tap_us(far, 1), tap_us(far + 500, 2)],
    );
    let out = note_rows(&chart);
    assert_eq!(out.rows.len(), 2);
    assert_eq!(out.rows[1].notes, 0b0110);
    assert!(out.rows.windows(2).all(|w| w[0].time_s < w[1].time_s));
}

#[test]
fn masks_stay_inside_the_keymode() {
    let k4 = chart![step = 100; "xxxx", "[..x", "]xx."];
    let k7 = chart![step = 100; "xxxxxxx", "[.....x", "]xxxxx."];
    let out4 = note_rows(&k4);
    let out7 = note_rows(&k7);
    assert!(out4.rows.iter().all(|r| r.notes != 0 && r.notes <= 0xF));
    assert!(out7.rows.iter().all(|r| r.notes != 0 && r.notes <= 0x7F));
    assert_eq!(out4.rows[0].notes, 0xF);
    assert_eq!(out7.rows[0].notes, 0x7F);
}

#[test]
fn hold_share_is_the_ln_share_of_objects() {
    let chart = chart![step = 100;
        "[xx[",
        "|..|",
        "]..]",
        "x.x.",
    ];
    // 2 LNs among 6 objects.
    assert_eq!(note_rows(&chart).hold_share_permille, 333);
    let rice = chart![step = 100; "x...", ".x.."];
    assert_eq!(note_rows(&rice).hold_share_permille, 0);
}

#[test]
fn empty_chart_has_no_rows() {
    let out = note_rows(&build(Keymode::K4, Vec::new()));
    assert!(out.rows.is_empty());
    assert_eq!(out.hold_share_permille, 0);
}

#[test]
fn rice_chart_is_rated_at_every_grid_rate() {
    let params = MinaCalcParams::default();
    let table = msd_table(&mut calc(), &stream_4k(180, 30), &params);
    assert_eq!(table.status, MsdStatus::Rated);
    assert_eq!(table.hold_share_permille, 0);
    let rates: Vec<u16> = table.rows.iter().map(|r| r.rate_milli).collect();
    assert_eq!(rates, params.rate_grid_milli);
}

#[test]
fn synthetic_stream_has_sane_skillsets() {
    let params = MinaCalcParams {
        rate_grid_milli: vec![1000],
        ..MinaCalcParams::default()
    };
    let table = msd_table(&mut calc(), &stream_4k(180, 30), &params);
    let centi = table.rows[0].centi;
    assert!(centi.iter().all(|&v| v > 0), "{centi:?}");
    // The band is wide on purpose: it guards against unit slips (ms vs s, rate inversion), not
    // against libm bits.
    assert!((1000..=2600).contains(&centi[0]), "overall {}", centi[0]);
    // A chordless stream: v527 rates it as much Technical as Stream, but never as chords.
    let [_, stream, jumpstream, handstream, _, _, chordjack, _] = centi;
    assert!(
        stream > jumpstream + 300 && stream > handstream + 300 && stream > chordjack + 300,
        "{centi:?}"
    );
}

#[test]
fn seven_key_chart_is_rated() {
    let params = MinaCalcParams {
        rate_grid_milli: vec![1000],
        ..MinaCalcParams::default()
    };
    let table = msd_table(&mut calc(), &stream(Keymode::K7, 180, 30), &params);
    assert_eq!(table.status, MsdStatus::Rated);
    assert!(table.rows[0].centi.iter().all(|&v| v > 0), "{table:?}");
}

#[test]
fn ln_heavy_chart_is_unrated_without_calling_the_calc() {
    let chart = chart![step = 100;
        "[x[.",
        "|.|[",
        "]x]|",
        "...]",
    ];
    let table = msd_table(&mut calc(), &chart, &MinaCalcParams::default());
    assert_eq!(table.status, MsdStatus::Unrated(UnratedReason::LnHeavy));
    assert_eq!(table.hold_share_permille, 600);
    assert!(table.rows.is_empty());
}

#[test]
fn chart_the_calc_rejects_is_unrated() {
    let three_keys = chart![step = 100; "x..", ".x.", "..x"];
    let table = msd_table(&mut calc(), &three_keys, &MinaCalcParams::default());
    assert_eq!(
        table.status,
        MsdStatus::Unrated(UnratedReason::CalcRejected)
    );
    assert!(table.rows.is_empty());

    let empty = build(Keymode::K4, Vec::new());
    let table = msd_table(&mut calc(), &empty, &MinaCalcParams::default());
    assert_eq!(
        table.status,
        MsdStatus::Unrated(UnratedReason::CalcRejected)
    );
}

#[test]
fn msd_table_is_deterministic() {
    let chart = stream_4k(170, 20);
    let params = MinaCalcParams::default();
    let mut calc = calc();
    assert_eq!(
        msd_table(&mut calc, &chart, &params),
        msd_table(&mut calc, &chart, &params)
    );
}

#[test]
fn ssr_rises_with_the_goal() {
    let rows = note_rows(&stream_4k(180, 30)).rows;
    let mut calc = calc();
    let low = ssr_centi(&mut calc, &rows, 1000, 0.90, 4).unwrap();
    let high = ssr_centi(&mut calc, &rows, 1000, 0.96, 4).unwrap();
    assert!(low.iter().all(|&v| v > 0), "{low:?}");
    assert!(high[0] > low[0], "{low:?} {high:?}");
}

#[test]
fn ssr_passes_calc_errors_through() {
    let mut calc = calc();
    assert_eq!(
        ssr_centi(&mut calc, &[], 1000, 0.93, 4),
        Err(CalcError::Empty)
    );
    let rows = [NoteRow {
        notes: 1,
        time_s: 0.0,
    }];
    assert_eq!(
        ssr_centi(&mut calc, &rows, 1000, 0.93, 3),
        Err(CalcError::UnsupportedKeycount(3))
    );
}

/// `width` random distinct columns per row on 7K, `rows_per_s` rows a second.
fn chords_7k(rows_per_s: i64, seconds: i64, width: u32) -> Chart {
    let step_us = 1_000_000 / rows_per_s;
    let mut seed: u32 = 777;
    let mut notes = Vec::new();
    for i in 0..(seconds * rows_per_s) {
        let mut mask = 0u32;
        while mask.count_ones() < width {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            mask |= 1 << ((seed >> 16) % 7);
        }
        for col in 0..7u8 {
            if mask & (1 << col) != 0 {
                notes.push(tap_us(i * step_us, col));
            }
        }
    }
    build(Keymode::K7, notes)
}

/// 1200 BPM 16th jumps: v527's handstream search passes its ceiling from about 1.45x on.
fn jumps_7k_overflowing_at_high_rates() -> Chart {
    chords_7k(20, 30, 2)
}

/// Every searched skillset but jackspeed and technical overflows at every grid rate.
fn full_chords_7k_overflowing_everywhere() -> Chart {
    chords_7k(30, 30, 7)
}

fn searched(skillsets: [f32; 8]) -> Vec<f32> {
    SEARCHED_SKILLSETS.iter().map(|&i| skillsets[i]).collect()
}

#[test]
fn searched_skillsets_are_every_skillset_but_overall_and_stamina() {
    let ids: Vec<&str> = SEARCHED_SKILLSETS
        .iter()
        .map(|&i| wolluf_minacalc::SKILLSET_IDS[i])
        .collect();
    assert_eq!(
        ids,
        [
            "stream",
            "jumpstream",
            "handstream",
            "jackspeed",
            "chordjack",
            "technical"
        ]
    );
}

#[test]
fn a_result_with_a_zero_searched_skillset_or_a_non_finite_value_is_invalid() {
    let ok = [30.0, 20.0, 21.0, 22.0, 25.0, 18.0, 19.0, 0.2];
    assert_eq!(checked(Skillsets(ok)), Ok(Skillsets(ok)));
    for &i in &SEARCHED_SKILLSETS {
        let mut zero = ok;
        zero[i] = 0.0;
        assert_eq!(checked(Skillsets(zero)), Err(CalcError::Native), "{i}");
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for i in 0..8 {
            let mut v = ok;
            v[i] = bad;
            assert_eq!(checked(Skillsets(v)), Err(CalcError::Native), "{i} {bad}");
        }
    }
}

#[test]
fn chart_overflowing_at_high_rates_keeps_only_its_valid_rates() {
    let chart = jumps_7k_overflowing_at_high_rates();
    let params = MinaCalcParams::default();
    let mut calc = calc();
    // Precondition: the calc itself accepts the overflowing rate and hides it behind a 0.
    let rows = note_rows(&chart).rows;
    let raw_low = calc.msd(&rows, 0.7, 7).unwrap().0;
    let raw_high = calc.msd(&rows, 1.5, 7).unwrap().0;
    assert!(searched(raw_low).iter().all(|&v| v > 0.0), "{raw_low:?}");
    assert!(searched(raw_high).contains(&0.0), "{raw_high:?}");

    let table = msd_table(&mut calc, &chart, &params);
    assert_eq!(table.status, MsdStatus::Rated);
    let rates: Vec<u16> = table.rows.iter().map(|r| r.rate_milli).collect();
    assert!(rates.windows(2).all(|w| w[0] < w[1]), "{rates:?}");
    assert!(rates.iter().all(|r| params.rate_grid_milli.contains(r)));
    assert!(rates.contains(&700) && rates.contains(&1000), "{rates:?}");
    assert!(!rates.contains(&1500), "{rates:?}");
    for row in &table.rows {
        assert!(
            SEARCHED_SKILLSETS.iter().all(|&i| row.centi[i] > 0),
            "{row:?}"
        );
    }
}

#[test]
fn chart_with_no_valid_rate_is_calc_rejected() {
    let chart = full_chords_7k_overflowing_everywhere();
    let mut calc = calc();
    // Precondition: the calc accepts it with zeroed skillsets rather than rejecting it.
    let raw = calc.msd(&note_rows(&chart).rows, 1.0, 7).unwrap().0;
    assert!(searched(raw).contains(&0.0), "{raw:?}");

    let table = msd_table(&mut calc, &chart, &MinaCalcParams::default());
    assert_eq!(
        table.status,
        MsdStatus::Unrated(UnratedReason::CalcRejected)
    );
    assert!(table.rows.is_empty());
}

#[test]
fn ssr_of_an_overflowing_chart_is_an_error() {
    let rows = note_rows(&full_chords_7k_overflowing_everywhere()).rows;
    let mut calc = calc();
    assert!(calc.ssr(&rows, 1.0, 0.93, 7).is_ok());
    assert_eq!(
        ssr_centi(&mut calc, &rows, 1000, 0.93, 7),
        Err(CalcError::Native)
    );
    let jumps = note_rows(&jumps_7k_overflowing_at_high_rates()).rows;
    assert!(ssr_centi(&mut calc, &jumps, 700, 0.93, 7).is_ok());
    assert_eq!(
        ssr_centi(&mut calc, &jumps, 1500, 0.93, 7),
        Err(CalcError::Native)
    );
}

/// Map-length charts (about 1.5–3.5 min): on charts under a minute v527 itself is not monotone
/// in rate (adjacent-rate drops up to 1.18 MSD on 80–400 rows, none from 600 rows on).
fn arb_dense_4k() -> impl Strategy<Value = Chart> {
    prop::collection::vec((1u16..=15, 60i64..=180), 800..1200).prop_map(|steps| {
        let mut t = 0;
        let mut notes = Vec::new();
        for (mask, gap_ms) in steps {
            for col in 0..4u8 {
                if mask & (1 << col) != 0 {
                    notes.push(tap_us(t, col));
                }
            }
            t += gap_ms * 1_000;
        }
        build(Keymode::K4, notes)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn overall_msd_does_not_fall_as_rate_rises(chart in arb_dense_4k()) {
        let table = msd_table(&mut calc(), &chart, &MinaCalcParams::default());
        prop_assert_eq!(table.status, MsdStatus::Rated);
        for w in table.rows.windows(2) {
            prop_assert!(
                w[1].centi[0] >= w[0].centi[0],
                "overall falls from {} at {} to {} at {}",
                w[0].centi[0], w[0].rate_milli, w[1].centi[0], w[1].rate_milli
            );
        }
    }
}

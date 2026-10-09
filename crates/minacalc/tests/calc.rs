#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wolluf_minacalc::{CALC_VERSION, Calc, CalcError, NoteRow, SKILLSET_IDS, Skillsets};

/// 1/4 snap (16ths) at `bpm` for `seconds`, cycling through `pattern` masks.
fn chart(pattern: &[u32], bpm: f32, seconds: f32) -> Vec<NoteRow> {
    let step = 60.0 / bpm / 4.0;
    let count = (seconds / step) as usize;
    (0..count)
        .map(|i| NoteRow {
            notes: pattern[i % pattern.len()],
            time_s: i as f32 * step,
        })
        .collect()
}

/// A pure 1-3-2-4 roll would not do: MinaCalc nerfs rolls as they speed up, so its MSD
/// falls between rates 1.2 and 1.3. A jack-free pseudo-random stream rises with rate.
fn stream_4k() -> Vec<NoteRow> {
    let mut seed: u32 = 12_345;
    let mut last = u32::MAX;
    let pattern: Vec<u32> = (0..64)
        .map(|_| {
            loop {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let col = (seed >> 16) % 4;
                if col != last {
                    last = col;
                    return 1 << col;
                }
            }
        })
        .collect();
    chart(&pattern, 180.0, 30.0)
}

fn stream_7k() -> Vec<NoteRow> {
    chart(
        &[
            0b000_0001, 0b001_0000, 0b000_0100, 0b100_0000, 0b000_0010, 0b010_0000, 0b000_1000,
            0b100_0001,
        ],
        160.0,
        30.0,
    )
}

fn assert_all_positive(s: &Skillsets) {
    for (id, v) in SKILLSET_IDS.iter().zip(s.0) {
        assert!(v > 0.0, "{id} = {v} is not positive in {s:?}");
    }
}

#[test]
fn version_is_527_on_both_sides() {
    assert_eq!(CALC_VERSION, 527);
    assert_eq!(Calc::version(), 527);
}

#[test]
fn skillset_ids_follow_upstream_order() {
    assert_eq!(
        SKILLSET_IDS,
        [
            "overall",
            "stream",
            "jumpstream",
            "handstream",
            "stamina",
            "jackspeed",
            "chordjack",
            "technical"
        ]
    );
}

#[test]
fn stream_4k_msd_and_ssr_are_positive() {
    let mut calc = Calc::new().unwrap();
    let rows = stream_4k();
    assert_all_positive(&calc.msd(&rows, 1.0, 4).unwrap());
    assert_all_positive(&calc.ssr(&rows, 1.0, 0.93, 4).unwrap());
}

#[test]
fn msd_overall_rises_with_rate() {
    let mut calc = Calc::new().unwrap();
    let rows = stream_4k();
    let at_1_0 = calc.msd(&rows, 1.0, 4).unwrap();
    let at_1_2 = calc.msd(&rows, 1.2, 4).unwrap();
    assert!(at_1_2.0[0] > at_1_0.0[0], "{at_1_2:?} vs {at_1_0:?}");
}

#[test]
fn stream_7k_is_rated() {
    let mut calc = Calc::new().unwrap();
    let s = calc.msd(&stream_7k(), 1.0, 7).unwrap();
    assert!(s.0[0] > 0.0, "{s:?}");
    let s = calc.ssr(&stream_7k(), 1.0, 0.93, 7).unwrap();
    assert!(s.0[0] > 0.0, "{s:?}");
}

#[test]
fn every_supported_keycount_rates_a_stream() {
    let mut calc = Calc::new().unwrap();
    for k in 4u8..=10 {
        let pattern: Vec<u32> = (0..u32::from(k)).map(|c| 1 << c).collect();
        let s = calc.msd(&chart(&pattern, 170.0, 20.0), 1.0, k).unwrap();
        assert!(s.0[0] > 0.0, "{k}K: {s:?}");
    }
}

#[test]
fn identical_runs_give_identical_bits() {
    let rows = stream_4k();
    let mut a = Calc::new().unwrap();
    let mut b = Calc::new().unwrap();
    let bits = |s: Skillsets| s.0.map(f32::to_bits);
    let first = bits(a.msd(&rows, 1.1, 4).unwrap());
    // A Calc reused after another chart must not carry state over.
    a.msd(&stream_7k(), 1.3, 7).unwrap();
    assert_eq!(first, bits(a.msd(&rows, 1.1, 4).unwrap()));
    assert_eq!(first, bits(b.msd(&rows, 1.1, 4).unwrap()));
    let s1 = bits(a.ssr(&rows, 1.0, 0.95, 4).unwrap());
    assert_eq!(s1, bits(b.ssr(&rows, 1.0, 0.95, 4).unwrap()));
}

#[test]
fn empty_input_is_rejected() {
    let mut calc = Calc::new().unwrap();
    assert_eq!(calc.msd(&[], 1.0, 4), Err(CalcError::Empty));
    assert_eq!(calc.ssr(&[], 1.0, 0.93, 4), Err(CalcError::Empty));
}

#[test]
fn non_increasing_times_are_rejected() {
    let mut calc = Calc::new().unwrap();
    let mut rows = stream_4k();
    rows[10].time_s = rows[9].time_s;
    assert_eq!(
        calc.msd(&rows, 1.0, 4),
        Err(CalcError::NotIncreasing { index: 10 })
    );
    let mut rows = stream_4k();
    rows[3].time_s = f32::NAN;
    assert_eq!(
        calc.ssr(&rows, 1.0, 0.93, 4),
        Err(CalcError::NotIncreasing { index: 3 })
    );
    let mut rows = stream_4k();
    rows[0].time_s = -0.5;
    assert_eq!(
        calc.msd(&rows, 1.0, 4),
        Err(CalcError::NotIncreasing { index: 0 })
    );
}

#[test]
fn masks_outside_the_keycount_are_rejected() {
    let mut calc = Calc::new().unwrap();
    let mut rows = stream_4k();
    rows[5].notes = 0;
    assert_eq!(
        calc.msd(&rows, 1.0, 4),
        Err(CalcError::MaskOutOfRange { index: 5 })
    );
    let mut rows = stream_4k();
    rows[7].notes = 1 << 4;
    assert_eq!(
        calc.msd(&rows, 1.0, 4),
        Err(CalcError::MaskOutOfRange { index: 7 })
    );
    let mut rows = stream_7k();
    rows[2].notes = 0b111_1111;
    assert!(calc.msd(&rows, 1.0, 7).is_ok());
}

#[test]
fn unsupported_keycounts_are_rejected() {
    let mut calc = Calc::new().unwrap();
    for k in [0u8, 1, 2, 3, 11, 18, 255] {
        assert_eq!(
            calc.msd(&stream_4k(), 1.0, k),
            Err(CalcError::UnsupportedKeycount(k))
        );
    }
}

#[test]
fn native_rejections_map_to_native() {
    let mut calc = Calc::new().unwrap();
    // 60 rows inside one 0.5 s interval: upstream skips the file as a joke chart.
    let burst: Vec<NoteRow> = (0..60)
        .map(|i| NoteRow {
            notes: 1 << (i % 4),
            time_s: i as f32 * 0.005,
        })
        .collect();
    assert_eq!(calc.msd(&burst, 1.0, 4), Err(CalcError::Native));
    let single = [NoteRow {
        notes: 1,
        time_s: 0.0,
    }];
    assert_eq!(calc.msd(&single, 1.0, 4), Err(CalcError::Native));
    for rate in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(calc.msd(&stream_4k(), rate, 4), Err(CalcError::Native));
    }
    for goal in [0.0, f32::NAN] {
        assert_eq!(calc.ssr(&stream_4k(), 1.0, goal, 4), Err(CalcError::Native));
    }
    // Past upstream's 100000-interval cap the float-to-int cast would be UB natively.
    let long = [
        NoteRow {
            notes: 1,
            time_s: 0.0,
        },
        NoteRow {
            notes: 2,
            time_s: 1.0e9,
        },
    ];
    assert_eq!(calc.msd(&long, 1.0, 4), Err(CalcError::Native));
    // The Calc stays usable after a rejection.
    assert!(calc.msd(&stream_4k(), 1.0, 4).is_ok());
}

#[test]
fn errors_display_their_cause() {
    assert_eq!(CalcError::Empty.to_string(), "no note rows");
    assert_eq!(
        CalcError::NotIncreasing { index: 3 }.to_string(),
        "row 3: time is not finite, negative, or not after the previous row"
    );
    assert_eq!(
        CalcError::MaskOutOfRange { index: 4 }.to_string(),
        "row 4: note mask is empty or names a column past the keycount"
    );
    assert_eq!(
        CalcError::UnsupportedKeycount(3).to_string(),
        "MinaCalc does not support 3K"
    );
    assert_eq!(
        CalcError::Native.to_string(),
        "MinaCalc rejected the chart or failed"
    );
    let e: &dyn std::error::Error = &CalcError::Native;
    assert!(e.source().is_none());
}

#[test]
fn calc_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Calc>();
}

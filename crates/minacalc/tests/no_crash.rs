#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use wolluf_minacalc::{Calc, CalcError, NoteRow};

/// Well-formed charts for `keycount`, so most cases reach the native side.
fn valid_rows(keycount: u8) -> impl Strategy<Value = Vec<NoteRow>> {
    let full = (1u32 << keycount.clamp(1, 31)) - 1;
    prop::collection::vec((1u32..=full.max(1), 0.0005f32..0.4), 0..400).prop_map(|v| {
        let mut t = 0.0;
        v.into_iter()
            .map(|(notes, gap)| {
                t += gap;
                NoteRow { notes, time_s: t }
            })
            .collect()
    })
}

fn any_time() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => 0.0f32..600.0,
        1 => any::<f32>(),
        1 => Just(f32::NAN),
    ]
}

fn arbitrary_rows() -> impl Strategy<Value = Vec<NoteRow>> {
    prop::collection::vec(
        (any::<u32>(), any_time()).prop_map(|(notes, time_s)| NoteRow { notes, time_s }),
        0..200,
    )
}

fn case() -> impl Strategy<Value = (u8, Vec<NoteRow>)> {
    (0u8..12).prop_flat_map(|k| {
        let rows = prop_oneof![3 => valid_rows(k), 1 => arbitrary_rows()];
        (Just(k), rows)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn never_crashes((keycount, rows) in case(),
                     rate in prop_oneof![4 => 0.5f32..2.0, 1 => any::<f32>()],
                     goal in prop_oneof![4 => 0.8f32..1.0, 1 => any::<f32>()]) {
        let mut calc = Calc::new().unwrap();
        let msd = calc.msd(&rows, rate, keycount);
        let ssr = calc.ssr(&rows, rate, goal, keycount);
        if rows.is_empty() {
            prop_assert_eq!(msd, Err(CalcError::Empty));
            prop_assert_eq!(ssr, Err(CalcError::Empty));
        }
        for s in [msd, ssr].into_iter().flatten() {
            prop_assert!(s.0.iter().all(|v| v.is_finite() && *v >= 0.0), "{:?}", s);
        }
    }
}

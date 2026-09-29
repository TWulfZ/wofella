//! `cargo xtask fixtures synthetic`: `.osr` files built from the source-osu test encoders, one per
//! header/payload shape the decoder branches on (spec 002 Design, "Fixtures").

use anyhow::Context;
use wolluf_source_osu::codec::OsuString;
use wolluf_source_osu::codec::osr::decode_osr;
use wolluf_source_osu::codec::score_header::mods;
use wolluf_source_osu::testkit::{OsrBuilder, ScoreBuilder, encode_osr};

use super::OutFile;
use super::dbs::fixture_md5;

/// LZMA_ALONE properties byte 0x5d, as every stable replay starts; the rest is opaque filler that
/// no F0 code decompresses.
const OPAQUE_PAYLOAD: [u8; 16] = [
    0x5d, 0x00, 0x00, 0x20, 0x00, 0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a,
];
/// Fixture md5 numbers stay clear of the 1.. range `fixtures dbs` hands out.
const BEATMAP_MD5_BASE: usize = 900;
const REPLAY_MD5_BASE: usize = 950;
const ONLINE_ID_BASE: i64 = 1000;
const STD_MODE: u8 = 0;
const TARGET_PRACTICE_VALUE: f64 = 0.875;
const LIFE_BAR: &str = "0|1,15000|0.95,30000|1,";

struct Case {
    score: ScoreBuilder,
    payload: Option<&'static [u8]>,
}

fn cases() -> Vec<Case> {
    let base = |nth: usize, player: &str| {
        ScoreBuilder::mania(
            &fixture_md5(BEATMAP_MD5_BASE + nth),
            player,
            i64::try_from(nth).unwrap_or_default(),
        )
    };
    let online_id = |nth: i64| ONLINE_ID_BASE + nth;
    vec![
        Case {
            score: base(1, "player-01").life_bar(OsuString::present(LIFE_BAR)),
            payload: Some(&OPAQUE_PAYLOAD),
        },
        // Mirrors the 3 pilot files of 126 bytes: declared length 0 with a non-zero online id.
        Case {
            score: base(2, "player-02").online_id(online_id(2)),
            payload: None,
        },
        Case {
            score: base(3, "player-01")
                .mode(STD_MODE)
                .mods(mods::TARGET_PRACTICE)
                .target_practice(TARGET_PRACTICE_VALUE),
            payload: Some(&OPAQUE_PAYLOAD),
        },
        Case {
            score: base(4, "").mods(mods::SCORE_V2).online_id(online_id(4)),
            payload: Some(&OPAQUE_PAYLOAD),
        },
    ]
}

pub(crate) fn generate() -> anyhow::Result<Vec<OutFile>> {
    cases()
        .into_iter()
        .enumerate()
        .map(|(i, case)| {
            let name = OsrBuilder::new(case.score.clone())
                .file_name()
                .context("synthetic score has no Data/r name")?;
            let mut record = case.score.build();
            // ScoreBuilder's default replay md5 is a counter, which the anonymization test rejects.
            record.header.replay_md5 = OsuString::present(fixture_md5(REPLAY_MD5_BASE + i + 1));
            let bytes = encode_osr(
                &record.header,
                case.payload,
                record.online_id,
                record.target_practice,
            );
            decode_osr(&bytes).context("synthetic .osr does not decode")?;
            Ok(OutFile {
                path: format!("osr/{}", name.format()),
                bytes,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolluf_source_osu::DiagCode;
    use wolluf_source_osu::codec::osr::{check_name_consistency, decode_osr};
    use wolluf_source_osu::codec::replay_name::ReplayFileName;

    #[test]
    fn synthetic_set_covers_the_four_shapes() {
        let files = generate().unwrap();
        assert_eq!(files.len(), 4);
        let mut empty = 0;
        let mut target_practice = 0;
        let mut v2_online = 0;
        let mut opaque = 0;
        for f in &files {
            let name = f.path.strip_prefix("osr/").unwrap();
            let parsed = ReplayFileName::parse(name).unwrap();
            let (osr, diags) = decode_osr(&f.bytes).unwrap();
            assert!(
                check_name_consistency(&parsed, &osr.header).is_empty(),
                "{name}"
            );
            match &osr.payload {
                None => {
                    assert!(diags.contains(DiagCode::OsrEmptyPayload));
                    empty += 1;
                }
                Some(span) if f.bytes[span.clone()] == OPAQUE_PAYLOAD => opaque += 1,
                Some(_) => {}
            }
            if osr.target_practice.is_some() && osr.header.mode == 0 {
                target_practice += 1;
            }
            if osr.header.is_score_v2() && osr.online_id.positive().is_some() {
                v2_online += 1;
            }
        }
        assert_eq!((empty, target_practice, v2_online), (1, 1, 1));
        assert!(opaque >= 1);
    }

    #[test]
    fn synthetic_generation_is_deterministic() {
        assert_eq!(generate().unwrap(), generate().unwrap());
    }
}

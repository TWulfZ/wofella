//! Per-record judgement events derived from cumulative `.osg` counts (spec 006 Design). A record
//! carries no column or object index, so assigning events to notes is F2's job (H4, C2).

use crate::codec::osg::OsgFile;
use crate::codec::score_header::JudgementCounts;
use crate::stable::stable_str_enum;

stable_str_enum! {
    /// mania judgements in the fixed dump order MAX, 300, 200, 100, 50, miss (spec 006).
    pub enum JudgementKind {
        Max => "MAX",
        N300 => "300",
        N200 => "200",
        N100 => "100",
        N50 => "50",
        Miss => "miss",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsgEvent {
    pub idx: usize,
    pub t_ms: i32,
    /// Judgements added by this record; a count that went down contributes 0 (and is already
    /// reported as `osg.count_decreases`).
    pub added: JudgementCounts,
    pub n: u16,
    pub combo_delta: i32,
    pub score_delta: i32,
}

impl OsgEvent {
    pub fn kinds(&self) -> Vec<JudgementKind> {
        let a = &self.added;
        [
            (JudgementKind::Max, a.geki),
            (JudgementKind::N300, a.n300),
            (JudgementKind::N200, a.katu),
            (JudgementKind::N100, a.n100),
            (JudgementKind::N50, a.n50),
            (JudgementKind::Miss, a.miss),
        ]
        .into_iter()
        .flat_map(|(kind, n)| std::iter::repeat_n(kind, usize::from(n)))
        .collect()
    }

    /// Typically the last record: a small score adjustment with no judgement.
    pub const fn is_score_only(&self) -> bool {
        self.n == 0
    }
}

fn delta(now: &JudgementCounts, before: &JudgementCounts) -> JudgementCounts {
    JudgementCounts {
        n300: now.n300.saturating_sub(before.n300),
        n100: now.n100.saturating_sub(before.n100),
        n50: now.n50.saturating_sub(before.n50),
        geki: now.geki.saturating_sub(before.geki),
        katu: now.katu.saturating_sub(before.katu),
        miss: now.miss.saturating_sub(before.miss),
    }
}

pub fn events(osg: &OsgFile) -> Vec<OsgEvent> {
    let zero = JudgementCounts::default();
    let mut prev_counts = &zero;
    let (mut prev_combo, mut prev_score) = (0i32, 0i32);
    osg.records
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let added = delta(&r.counts, prev_counts);
            let n = u16::try_from(added.total()).unwrap_or(u16::MAX);
            let event = OsgEvent {
                idx,
                t_ms: r.t_ms,
                added,
                n,
                combo_delta: i32::from(r.combo) - prev_combo,
                score_delta: r.score.saturating_sub(prev_score),
            };
            prev_counts = &r.counts;
            prev_combo = i32::from(r.combo);
            prev_score = r.score;
            event
        })
        .collect()
}

pub fn final_state(osg: &OsgFile) -> Option<(&JudgementCounts, i32)> {
    osg.records.last().map(|r| (&r.counts, r.score))
}

/// H3: the last record should equal the `.osr` header. A file without records matches nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalCheck {
    pub counts_eq: bool,
    pub score_eq: bool,
}

pub fn check_final(
    osg: &OsgFile,
    header_counts: &JudgementCounts,
    header_score: i32,
) -> FinalCheck {
    match final_state(osg) {
        Some((counts, score)) => FinalCheck {
            counts_eq: counts == header_counts,
            score_eq: score == header_score,
        },
        None => FinalCheck {
            counts_eq: false,
            score_eq: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::osg::{OsgRecord, OsgScoreSystem};

    fn rec(t_ms: i32, counts: JudgementCounts, score: i32, combo: u16) -> OsgRecord {
        OsgRecord {
            t_ms,
            counts,
            score,
            max_combo: combo,
            combo,
            hp_raw: 200,
            b4: 0,
            b25: 0,
            b28: 0,
            v2: None,
        }
    }

    fn file(records: Vec<OsgRecord>) -> OsgFile {
        OsgFile {
            client_version: 20_260_924,
            score_system: Some(OsgScoreSystem::V1),
            records,
        }
    }

    fn counts(geki: u16, n300: u16, katu: u16, miss: u16) -> JudgementCounts {
        JudgementCounts {
            geki,
            n300,
            katu,
            miss,
            ..JudgementCounts::default()
        }
    }

    #[test]
    fn single_judgement_per_record() {
        let osg = file(vec![
            rec(100, counts(1, 0, 0, 0), 320, 1),
            rec(250, counts(1, 1, 0, 0), 620, 2),
            rec(400, counts(1, 1, 1, 0), 820, 3),
        ]);
        let ev = events(&osg);
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].kinds(), vec![JudgementKind::Max]);
        assert_eq!(ev[1].kinds(), vec![JudgementKind::N300]);
        assert_eq!(ev[2].kinds(), vec![JudgementKind::N200]);
        assert!(ev.iter().all(|e| e.n == 1 && e.combo_delta == 1));
        assert_eq!((ev[1].idx, ev[1].t_ms, ev[1].score_delta), (1, 250, 300));
        assert_eq!(ev[0].added, counts(1, 0, 0, 0));
    }

    #[test]
    fn merged_chord_record_has_n2() {
        let osg = file(vec![
            rec(100, counts(0, 0, 0, 0), 0, 0),
            rec(500, counts(1, 1, 0, 0), 620, 2),
        ]);
        let ev = events(&osg);
        assert_eq!(ev[1].n, 2);
        assert_eq!(ev[1].kinds(), vec![JudgementKind::Max, JudgementKind::N300]);
        assert_eq!(ev[1].combo_delta, 2);
        let kinds: Vec<&str> = ev[1].kinds().iter().map(|k| k.as_str()).collect();
        assert_eq!(kinds, ["MAX", "300"]);
    }

    #[test]
    fn score_only_record_has_n0() {
        let osg = file(vec![
            rec(100, counts(1, 0, 0, 0), 320, 1),
            rec(100, counts(1, 0, 0, 0), 325, 1),
        ]);
        let ev = events(&osg);
        assert_eq!(ev[1].n, 0);
        assert!(ev[1].kinds().is_empty());
        assert_eq!(ev[1].score_delta, 5);
        assert!(ev[1].is_score_only());
        // A miss breaks combo: a negative delta must survive.
        let broken = file(vec![
            rec(1, counts(5, 0, 0, 0), 1_600, 5),
            rec(2, counts(5, 0, 0, 1), 1_600, 0),
        ]);
        assert_eq!(events(&broken)[1].combo_delta, -5);
    }

    #[test]
    fn final_check_matches_header() {
        let last = counts(3, 2, 1, 0);
        let osg = file(vec![
            rec(1, counts(1, 0, 0, 0), 300, 1),
            rec(2, last, 1_900, 6),
        ]);
        assert_eq!(final_state(&osg), Some((&last, 1_900)));
        assert_eq!(
            check_final(&osg, &last, 1_900),
            FinalCheck {
                counts_eq: true,
                score_eq: true
            }
        );
    }

    #[test]
    fn final_check_detects_mismatch() {
        let osg = file(vec![rec(1, counts(3, 2, 1, 0), 1_900, 6)]);
        let header = counts(4, 2, 1, 0);
        assert_eq!(
            check_final(&osg, &header, 1_900),
            FinalCheck {
                counts_eq: false,
                score_eq: true
            }
        );
        assert_eq!(
            check_final(&osg, &counts(3, 2, 1, 0), 2_000),
            FinalCheck {
                counts_eq: true,
                score_eq: false
            }
        );
        let empty = file(vec![]);
        assert_eq!(final_state(&empty), None);
        assert_eq!(
            check_final(&empty, &header, 0),
            FinalCheck {
                counts_eq: false,
                score_eq: false
            }
        );
    }
}

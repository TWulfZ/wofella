//! A play's judgement counts as the stable score header stores them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayCounts {
    pub max: u16,
    pub n300: u16,
    pub n200: u16,
    pub n100: u16,
    pub n50: u16,
    pub miss: u16,
}

impl PlayCounts {
    pub fn total(&self) -> u32 {
        [
            self.max, self.n300, self.n200, self.n100, self.n50, self.miss,
        ]
        .into_iter()
        .map(u32::from)
        .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreSystem {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    Incomplete,
}

impl Completeness {
    /// `n_objects` counts taps and LNs; `n_ln` counts LNs alone.
    /// V1 judges an LN once, but some V1 plays carry split head and tail judgements (research
    /// 03 l.165), so V1 only needs one judgement per object; V2 always judges heads and tails.
    pub fn of(counts: &PlayCounts, score_system: ScoreSystem, n_objects: u32, n_ln: u32) -> Self {
        let needed = match score_system {
            ScoreSystem::V1 => n_objects,
            ScoreSystem::V2 => n_objects.saturating_add(n_ln),
        };
        let total = counts.total();
        if total > 0 && total >= needed {
            Self::Complete
        } else {
            Self::Incomplete
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(max: u16, miss: u16) -> PlayCounts {
        PlayCounts {
            max,
            n300: 3,
            n200: 2,
            n100: 1,
            n50: 1,
            miss,
        }
    }

    #[test]
    fn total_sums_every_judgement() {
        assert_eq!(counts(10, 3).total(), 20);
        let full = PlayCounts {
            max: u16::MAX,
            n300: u16::MAX,
            n200: u16::MAX,
            n100: u16::MAX,
            n50: u16::MAX,
            miss: u16::MAX,
        };
        assert_eq!(full.total(), 6 * u32::from(u16::MAX));
    }

    #[test]
    fn v1_is_complete_from_the_object_count() {
        // 100 objects, 30 of them LNs: V1 judges an LN once.
        assert_eq!(
            Completeness::of(&counts(93, 0), ScoreSystem::V1, 100, 30),
            Completeness::Complete
        );
        assert_eq!(
            Completeness::of(&counts(92, 0), ScoreSystem::V1, 100, 30),
            Completeness::Incomplete
        );
    }

    #[test]
    fn v1_with_split_ln_judgements_is_complete() {
        // Research 03 l.165: some V1 HR LN plays carry head and tail judgements.
        assert_eq!(
            Completeness::of(&counts(123, 0), ScoreSystem::V1, 100, 30),
            Completeness::Complete
        );
    }

    #[test]
    fn v2_needs_heads_and_tails() {
        assert_eq!(
            Completeness::of(&counts(123, 0), ScoreSystem::V2, 100, 30),
            Completeness::Complete
        );
        assert_eq!(
            Completeness::of(&counts(93, 0), ScoreSystem::V2, 100, 30),
            Completeness::Incomplete
        );
    }

    #[test]
    fn a_play_with_no_judgements_is_incomplete() {
        assert_eq!(
            Completeness::of(&PlayCounts::default(), ScoreSystem::V1, 1, 0),
            Completeness::Incomplete
        );
    }
}

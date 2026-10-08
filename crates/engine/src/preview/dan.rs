//! 4K dan estimate from Overall MSD (ADR 0024). The table is fitted by wolluf on public dan
//! courses (`docs/research/07-k4-dan-from-msd.md`); the default stays empty until it lands.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DanTable4k {
    /// Ascending by lower bound: `(label, lower_bound_centi)`.
    pub dans: Vec<(String, i32)>,
    /// The last dan has no upper neighbour, so its thirds use this span.
    pub top_span_centi: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DanThird {
    Low,
    Mid,
    High,
}

impl DanThird {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanEstimate {
    pub label: String,
    pub third: DanThird,
    /// Overall above the dan's lower bound.
    pub margin_centi: i32,
}

impl DanTable4k {
    pub fn estimate(&self, overall_centi: i32) -> Option<DanEstimate> {
        let i = self
            .dans
            .iter()
            .rposition(|(_, lower)| *lower <= overall_centi)?;
        let (label, lower) = &self.dans[i];
        let span = self
            .dans
            .get(i + 1)
            .map_or(self.top_span_centi, |(_, next)| next.saturating_sub(*lower));
        let margin = overall_centi.saturating_sub(*lower);
        let third = if margin.saturating_mul(3) < span {
            DanThird::Low
        } else if margin.saturating_mul(3) < span.saturating_mul(2) {
            DanThird::Mid
        } else {
            DanThird::High
        };
        Some(DanEstimate {
            label: label.clone(),
            third,
            margin_centi: margin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> DanTable4k {
        DanTable4k {
            dans: vec![("1st".into(), 1000), ("2nd".into(), 1300)],
            top_span_centi: 600,
        }
    }

    fn est(overall: i32) -> Option<(String, DanThird, i32)> {
        table()
            .estimate(overall)
            .map(|e| (e.label, e.third, e.margin_centi))
    }

    #[test]
    fn below_the_first_dan_there_is_no_estimate() {
        assert_eq!(est(999), None);
        assert_eq!(est(-5), None);
    }

    #[test]
    fn thirds_split_the_span_to_the_next_dan() {
        assert_eq!(est(1000), Some(("1st".into(), DanThird::Low, 0)));
        assert_eq!(est(1099), Some(("1st".into(), DanThird::Low, 99)));
        assert_eq!(est(1100), Some(("1st".into(), DanThird::Mid, 100)));
        assert_eq!(est(1199), Some(("1st".into(), DanThird::Mid, 199)));
        assert_eq!(est(1200), Some(("1st".into(), DanThird::High, 200)));
        assert_eq!(est(1299), Some(("1st".into(), DanThird::High, 299)));
        assert_eq!(est(1300), Some(("2nd".into(), DanThird::Low, 0)));
    }

    #[test]
    fn the_last_dan_uses_the_top_span_and_stays_high_beyond_it() {
        assert_eq!(est(1499), Some(("2nd".into(), DanThird::Low, 199)));
        assert_eq!(est(1500), Some(("2nd".into(), DanThird::Mid, 200)));
        assert_eq!(est(1700), Some(("2nd".into(), DanThird::High, 400)));
        assert_eq!(est(9000), Some(("2nd".into(), DanThird::High, 7700)));
    }

    #[test]
    fn the_default_table_is_empty_and_estimates_nothing() {
        let t = DanTable4k::default();
        assert!(t.dans.is_empty());
        assert_eq!(t.estimate(3000), None);
        assert_eq!(DanThird::High.as_str(), "high");
    }
}

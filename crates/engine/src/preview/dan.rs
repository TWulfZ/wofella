//! 4K dan estimate from Overall MSD (ADR 0024). The table is fitted by wolluf on public dan
//! courses (`docs/research/07-k4-dan-from-msd.md`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanTable4k {
    /// Ascending by lower bound: `(label, lower_bound_centi)`.
    pub dans: Vec<(String, i32)>,
    /// The last dan has no upper neighbour, so its thirds use this span.
    pub top_span_centi: i32,
}

/// Fitted by `research/scripts/dan4k/fit.py` on MinaCalc 527 Overall at 1.0x; method, data and
/// leave-one-pack-out error in `docs/research/07-k4-dan-from-msd.md`.
impl Default for DanTable4k {
    fn default() -> Self {
        let dans = [
            ("1st", 1320),
            ("2nd", 1517),
            ("3rd", 1616),
            ("4th", 1748),
            ("5th", 1965),
            ("6th", 2143),
            ("7th", 2278),
            ("8th", 2378),
            ("9th", 2488),
            ("10th", 2582),
            ("Alpha", 2681),
            ("Beta", 2770),
            ("Gamma", 2919),
            ("Delta", 3137),
            ("Epsilon", 3361),
        ];
        Self {
            dans: dans.iter().map(|&(l, b)| (l.to_owned(), b)).collect(),
            top_span_centi: 224,
        }
    }
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
    fn the_default_table_runs_from_1st_to_epsilon_with_rising_bounds() {
        let t = DanTable4k::default();
        let labels: Vec<&str> = t.dans.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "9th", "10th", "Alpha",
                "Beta", "Gamma", "Delta", "Epsilon"
            ]
        );
        assert!(t.dans.windows(2).all(|w| w[0].1 < w[1].1));
        assert!(t.top_span_centi > 0);
        assert_eq!(DanThird::High.as_str(), "high");
    }

    #[test]
    fn the_default_table_maps_msd_to_the_fitted_dans() {
        let est = |overall: i32| {
            DanTable4k::default()
                .estimate(overall)
                .map(|e| (e.label, e.third, e.margin_centi))
        };
        assert_eq!(est(1319), None);
        assert_eq!(est(1320), Some(("1st".into(), DanThird::Low, 0)));
        assert_eq!(est(2600), Some(("10th".into(), DanThird::Low, 18)));
        assert_eq!(est(2650), Some(("10th".into(), DanThird::High, 68)));
        assert_eq!(est(3000), Some(("Gamma".into(), DanThird::Mid, 81)));
        assert_eq!(est(3500), Some(("Epsilon".into(), DanThird::Mid, 139)));
        assert_eq!(est(4000), Some(("Epsilon".into(), DanThird::High, 639)));
    }
}

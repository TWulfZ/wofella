//! A labelling window within its chart: the chart's span, the `w+`/`w-`/`n`/`p` reshapes and
//! the timeline's moves.
//! Pure, so the sampler, submit and the shells agree on one definition of "inside the chart".

use wolluf_core::TimeUs;

/// Rows are integer milliseconds, so a window ending 1 ms after the last row still shows it
/// while staying half-open.
const LAST_ROW_SLACK: TimeUs = TimeUs::from_ms(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reshape {
    Widen,
    Narrow,
    Next,
    Prev,
}

/// `[first row, last row + 1 ms)`; `None` for a chart without rows.
pub fn chart_span(rows: &[TimeUs]) -> Option<(TimeUs, TimeUs)> {
    Some((*rows.first()?, TimeUs(rows.last()?.0 + LAST_ROW_SLACK.0)))
}

pub fn within(t0: TimeUs, t1: TimeUs, span: (TimeUs, TimeUs)) -> bool {
    span.0 <= t0 && t1 <= span.1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooShort;

/// Widen and narrow move the end by `step`; widening at the chart end grows the start
/// instead. Shifts move by half the window. Every result stays inside `span`; narrowing below
/// `min` is refused.
pub fn reshape(
    t0: TimeUs,
    t1: TimeUs,
    how: Reshape,
    span: (TimeUs, TimeUs),
    step: TimeUs,
    min: TimeUs,
) -> Result<(TimeUs, TimeUs), TooShort> {
    let (first, end) = span;
    let half = (t1.0 - t0.0) / 2;
    Ok(match how {
        Reshape::Widen => {
            let new_t1 = (t1.0 + step.0).min(end.0).max(t1.0);
            let rest = step.0 - (new_t1 - t1.0);
            (TimeUs((t0.0 - rest).max(first.0).min(t0.0)), TimeUs(new_t1))
        }
        Reshape::Narrow if t1.0 - t0.0 - step.0 < min.0 => return Err(TooShort),
        Reshape::Narrow => (t0, TimeUs(t1.0 - step.0)),
        Reshape::Next => {
            let shift = half.min(end.0 - t1.0).max(0);
            (TimeUs(t0.0 + shift), TimeUs(t1.0 + shift))
        }
        Reshape::Prev => {
            let shift = half.min(t0.0 - first.0).max(0);
            (TimeUs(t0.0 - shift), TimeUs(t1.0 - shift))
        }
    })
}

/// `[t0, t0 + len)` slid inside `span`; a window longer than the chart becomes the chart.
pub fn move_to(t0: TimeUs, len: TimeUs, span: (TimeUs, TimeUs)) -> (TimeUs, TimeUs) {
    let (first, end) = span;
    if len.0 >= end.0 - first.0 {
        return span;
    }
    let start = t0.0.clamp(first.0, end.0 - len.0);
    (TimeUs(start), TimeUs(start + len.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: i32) -> TimeUs {
        TimeUs::from_ms(v)
    }

    const SPAN: (TimeUs, TimeUs) = (TimeUs::from_ms(0), TimeUs::from_ms(10_001));

    fn go(t0: i32, t1: i32, how: Reshape) -> Result<(TimeUs, TimeUs), TooShort> {
        reshape(ms(t0), ms(t1), how, SPAN, ms(1_000), ms(1_000))
    }

    #[test]
    fn span_ends_one_ms_after_the_last_row() {
        assert_eq!(chart_span(&[ms(5), ms(10_000)]), Some((ms(5), ms(10_001))));
        assert_eq!(chart_span(&[]), None);
        assert!(within(ms(0), ms(10_001), SPAN));
        assert!(!within(ms(-1), ms(4_000), SPAN));
        assert!(!within(ms(7_000), ms(10_002), SPAN));
    }

    #[test]
    fn reshape_stays_inside_the_chart() {
        assert_eq!(go(1_000, 5_000, Reshape::Widen), Ok((ms(1_000), ms(6_000))));
        assert_eq!(
            go(6_001, 10_001, Reshape::Widen),
            Ok((ms(5_001), ms(10_001)))
        );
        assert_eq!(go(0, 10_001, Reshape::Widen), Ok((ms(0), ms(10_001))));
        assert_eq!(
            go(1_000, 5_000, Reshape::Narrow),
            Ok((ms(1_000), ms(4_000)))
        );
        assert_eq!(go(1_000, 2_000, Reshape::Narrow), Err(TooShort));
        assert_eq!(go(1_000, 5_000, Reshape::Next), Ok((ms(3_000), ms(7_000))));
        assert_eq!(go(5_000, 9_000, Reshape::Next), Ok((ms(6_001), ms(10_001))));
        assert_eq!(go(1_000, 5_000, Reshape::Prev), Ok((ms(0), ms(4_000))));
        assert_eq!(go(0, 4_000, Reshape::Prev), Ok((ms(0), ms(4_000))));
    }

    #[test]
    fn move_keeps_the_length_inside_the_chart() {
        let go = |t0: i32, len: i32| move_to(ms(t0), ms(len), SPAN);
        assert_eq!(go(2_000, 4_000), (ms(2_000), ms(6_000)));
        assert_eq!(go(-300, 4_000), (ms(0), ms(4_000)));
        assert_eq!(go(9_000, 4_000), (ms(6_001), ms(10_001)));
        assert_eq!(go(3_000, 20_000), SPAN);
    }
}

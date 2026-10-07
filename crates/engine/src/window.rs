//! What a playfield draws for one time window of a chart, as plain data: callers above the
//! engine map it to their DTOs without touching the chart model.

use wolluf_chart::{Chart, Finger, Hand, Layout, TimingKind};
use wolluf_core::TimeUs;

#[derive(Debug, Clone, PartialEq)]
pub struct ChartWindow {
    pub keymode: u8,
    /// Taps and LN heads in `[from, to]` plus LNs whose body enters from before `from`, by
    /// `(t_ms, col)`.
    pub notes: Vec<WindowNote>,
    /// The last red line at or before `from`, then every line in `[from, to]`.
    pub timing: Vec<TimingLine>,
    pub layout_id: String,
    /// One per column, leftmost first.
    pub columns: Vec<WindowColumn>,
    pub span: ChartSpan,
    pub audio_filename: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowNote {
    pub t_ms: i32,
    /// 0-based, column 0 leftmost.
    pub col: u8,
    /// The LN tail; `None` for a tap.
    pub end_ms: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimingLine {
    Red {
        t_ms: i32,
        beat_len_ms: f64,
        meter: u8,
    },
    Green {
        t_ms: i32,
        sv: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowColumn {
    pub hand: ColumnHand,
    pub finger: ColumnFinger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnHand {
    Left,
    Right,
    /// Either thumb may take it.
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnFinger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

/// The first and the last row of the chart (LN tails included); `0, 0` without rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartSpan {
    pub first_ms: i32,
    pub end_ms: i32,
}

/// `[from_ms, to_ms]`, both inclusive. No segments: the window feeds blind labelling.
pub fn chart_window(chart: &Chart, layout: &Layout, from_ms: i32, to_ms: i32) -> ChartWindow {
    let (from, to) = (TimeUs::from_ms(from_ms), TimeUs::from_ms(to_ms));
    let rows = chart.rows();
    let mut notes: Vec<WindowNote> = rows
        .iter()
        .filter(|r| from <= r.t && r.t <= to)
        .flat_map(|r| {
            (0..chart.keymode().columns())
                .filter(|&col| r.tap.contains(col))
                .map(|col| WindowNote {
                    t_ms: ms_i32(r.t),
                    col,
                    end_ms: None,
                })
        })
        .collect();
    notes.extend(
        chart
            .ln_pairs()
            .iter()
            .filter(|p| p.head <= to && (from <= p.head || from < p.tail))
            .map(|p| WindowNote {
                t_ms: ms_i32(p.head),
                col: p.col,
                end_ms: Some(ms_i32(p.tail)),
            }),
    );
    notes.sort_by_key(|n| (n.t_ms, n.col));

    let points = chart.timing();
    let red_before = points
        .iter()
        .rposition(|p| p.t <= from && matches!(p.kind, TimingKind::Uninherited { .. }));
    let timing = points
        .iter()
        .enumerate()
        .filter(|&(i, p)| Some(i) == red_before || (from <= p.t && p.t <= to))
        .map(|(_, p)| timing_line(p.t, p.kind))
        .collect();

    let audio = chart.meta().audio_filename.trim();
    ChartWindow {
        keymode: chart.keymode().columns(),
        notes,
        timing,
        layout_id: layout.id().to_owned(),
        columns: layout_columns(layout),
        span: ChartSpan {
            first_ms: rows.first().map_or(0, |r| ms_i32(r.t)),
            end_ms: rows.last().map_or(0, |r| ms_i32(r.t)),
        },
        audio_filename: (!audio.is_empty()).then(|| audio.to_owned()),
    }
}

/// One per column, leftmost first: what a playfield needs of a layout without a chart.
pub fn layout_columns(layout: &Layout) -> Vec<WindowColumn> {
    layout
        .columns()
        .iter()
        .map(|&(hand, finger)| WindowColumn {
            hand: column_hand(hand),
            finger: column_finger(finger),
        })
        .collect()
}

fn timing_line(t: TimeUs, kind: TimingKind) -> TimingLine {
    let t_ms = ms_i32(t);
    match kind {
        TimingKind::Uninherited { beat_len_ms, meter } => TimingLine::Red {
            t_ms,
            beat_len_ms,
            meter: u8::try_from(meter).unwrap_or(u8::MAX),
        },
        TimingKind::Inherited { sv } => TimingLine::Green { t_ms, sv },
    }
}

fn column_hand(hand: Hand) -> ColumnHand {
    match hand {
        Hand::Left => ColumnHand::Left,
        Hand::Right => ColumnHand::Right,
        Hand::Both => ColumnHand::Both,
    }
}

fn column_finger(finger: Finger) -> ColumnFinger {
    match finger {
        Finger::Pinky => ColumnFinger::Pinky,
        Finger::Ring => ColumnFinger::Ring,
        Finger::Middle => ColumnFinger::Middle,
        Finger::Index => ColumnFinger::Index,
        Finger::Thumb => ColumnFinger::Thumb,
    }
}

fn ms_i32(t: TimeUs) -> i32 {
    let ms = t.as_ms_floor();
    i32::try_from(ms).unwrap_or(if ms < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
    use wolluf_chart::{ChartMeta, Diagnostics, Note, NoteKind, TimingPoint};
    use wolluf_core::Keymode;

    use super::*;

    fn s(ms: i32) -> TimeUs {
        TimeUs::from_ms(ms)
    }

    fn red(t: i32, beat_len_ms: f64, meter: u32) -> TimingPoint {
        TimingPoint {
            t: s(t),
            kind: TimingKind::Uninherited { beat_len_ms, meter },
        }
    }

    fn green(t: i32, sv: f64) -> TimingPoint {
        TimingPoint {
            t: s(t),
            kind: TimingKind::Inherited { sv },
        }
    }

    fn tap(col: u8, t: i32) -> Note {
        Note {
            t: s(t),
            col,
            kind: NoteKind::Tap,
        }
    }

    fn hold(col: u8, t: i32, end: i32) -> Note {
        Note {
            t: s(t),
            col,
            kind: NoteKind::Hold { end: s(end) },
        }
    }

    fn k7(audio: &str, timing: Vec<TimingPoint>, notes: Vec<Note>) -> Chart {
        let meta = ChartMeta {
            audio_filename: audio.to_owned(),
            ..ChartMeta::default()
        };
        Chart::from_notes(Keymode::K7, meta, timing, notes, &mut Diagnostics::new())
    }

    fn sample() -> Chart {
        k7(
            "audio.mp3",
            vec![
                red(0, 500.0, 4),
                green(1_000, 2.0),
                red(2_000, 400.0, 3),
                green(2_500, 0.5),
                red(4_000, 300.0, 4),
            ],
            vec![
                tap(0, 1_000),
                tap(6, 2_200),
                tap(1, 2_500),
                tap(2, 2_500),
                tap(0, 3_000),
                tap(0, 3_200),
                hold(3, 1_500, 2_600),
                hold(4, 1_500, 2_100),
                hold(1, 1_800, 2_200),
                hold(2, 2_200, 2_400),
                hold(5, 2_800, 3_500),
                hold(6, 3_100, 3_300),
            ],
        )
    }

    fn layout() -> Layout {
        Layout::by_id("k7.313_right_thumb").unwrap()
    }

    fn note(t_ms: i32, col: u8, end_ms: Option<i32>) -> WindowNote {
        WindowNote { t_ms, col, end_ms }
    }

    #[test]
    fn notes_take_bounds_inclusive_and_bodies_entering_from_before() {
        let w = chart_window(&sample(), &layout(), 2_200, 3_000);
        assert_eq!(
            w.notes,
            [
                note(1_500, 3, Some(2_600)),
                note(2_200, 2, Some(2_400)),
                note(2_200, 6, None),
                note(2_500, 1, None),
                note(2_500, 2, None),
                note(2_800, 5, Some(3_500)),
                note(3_000, 0, None),
            ],
            "an LN ending before or at `from` and anything after `to` stay out; by (t, col)"
        );
        assert_eq!(w.keymode, 7);
    }

    #[test]
    fn timing_keeps_the_red_line_before_the_window_then_the_lines_inside() {
        let chart = sample();
        let w = chart_window(&chart, &layout(), 2_200, 3_000);
        assert_eq!(
            w.timing,
            [
                TimingLine::Red {
                    t_ms: 2_000,
                    beat_len_ms: 400.0,
                    meter: 3
                },
                TimingLine::Green {
                    t_ms: 2_500,
                    sv: 0.5
                },
            ],
            "the green line before the window is not carried in"
        );

        let at_from = chart_window(&chart, &layout(), 2_000, 2_100);
        assert_eq!(
            at_from.timing,
            [TimingLine::Red {
                t_ms: 2_000,
                beat_len_ms: 400.0,
                meter: 3
            }],
            "a red line at `from` is listed once"
        );

        let before_any = k7(
            "a.mp3",
            vec![green(100, 1.5), red(500, 300.0, 4)],
            vec![tap(0, 600)],
        );
        let early = chart_window(&before_any, &layout(), 0, 200);
        assert_eq!(early.timing, [TimingLine::Green { t_ms: 100, sv: 1.5 }]);
    }

    #[test]
    fn columns_follow_the_layout() {
        let w = chart_window(&sample(), &layout(), 0, 1);
        assert_eq!(w.layout_id, "k7.313_right_thumb");
        assert_eq!(w.columns.len(), 7);
        assert_eq!(
            w.columns[0],
            WindowColumn {
                hand: ColumnHand::Left,
                finger: ColumnFinger::Ring
            }
        );
        assert_eq!(
            w.columns[3],
            WindowColumn {
                hand: ColumnHand::Right,
                finger: ColumnFinger::Thumb
            }
        );

        let left = Layout::by_id("k7.313_left_thumb").unwrap();
        let w = chart_window(&sample(), &left, 0, 1);
        assert_eq!(w.layout_id, "k7.313_left_thumb");
        assert_eq!(w.columns[3].hand, ColumnHand::Left);
        assert_eq!(layout_columns(&left), w.columns);
    }

    #[test]
    fn span_runs_from_the_first_row_to_the_last_ln_tail() {
        let w = chart_window(&sample(), &layout(), 0, 1);
        assert_eq!(
            w.span,
            ChartSpan {
                first_ms: 1_000,
                end_ms: 3_500
            }
        );

        let empty = k7("a.mp3", vec![red(0, 500.0, 4)], Vec::new());
        let w = chart_window(&empty, &layout(), 0, 1_000);
        assert_eq!(
            w.span,
            ChartSpan {
                first_ms: 0,
                end_ms: 0
            }
        );
        assert!(w.notes.is_empty());
    }

    #[test]
    fn audio_filename_is_trimmed_and_none_when_blank() {
        let w = chart_window(&sample(), &layout(), 0, 1);
        assert_eq!(w.audio_filename.as_deref(), Some("audio.mp3"));

        let padded = k7(" song.ogg ", Vec::new(), vec![tap(0, 0)]);
        let w = chart_window(&padded, &layout(), 0, 1);
        assert_eq!(w.audio_filename.as_deref(), Some("song.ogg"));

        for blank in ["", "   "] {
            let silent = k7(blank, Vec::new(), vec![tap(0, 0)]);
            assert_eq!(chart_window(&silent, &layout(), 0, 1).audio_filename, None);
        }
    }
}

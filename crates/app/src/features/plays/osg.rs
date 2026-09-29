//! `.osg` spike tooling (spec 006): a read-only dump of one file and a structural survey of a
//! whole `Data/r`. Neither is an IPC surface, so nothing here derives specta (D13).

mod dump;

pub use dump::{
    DiagnosticRow, DumpFormat, DumpView, OsgDump, OsgDumpHeader, OsgEventRow, OsgRecordRow,
    inspect, render_diagnostics, render_dump,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use wolluf_core::ErrorCode;
    use wolluf_source_osu::codec::osg::{OsgFile, OsgRecord, OsgScoreSystem};
    use wolluf_source_osu::codec::score_header::JudgementCounts;
    use wolluf_source_osu::testkit::encode_osg;

    use super::*;

    const CLIENT: i32 = 20_260_924;

    fn counts(max: u16, n300: u16, n200: u16, miss: u16) -> JudgementCounts {
        JudgementCounts {
            geki: max,
            n300,
            katu: n200,
            miss,
            ..JudgementCounts::default()
        }
    }

    fn rec(
        t_ms: i32,
        c: JudgementCounts,
        score: i32,
        combo: u16,
        v2: Option<[f64; 2]>,
    ) -> OsgRecord {
        OsgRecord {
            t_ms,
            counts: c,
            score,
            max_combo: combo,
            combo,
            hp_raw: 200,
            b4: 0,
            b25: 0,
            b28: u8::from(v2.is_some()),
            v2,
        }
    }

    /// A single MAX, a two-note chord (MAX + 300), a 200, a miss that resets combo, and the
    /// trailing score-only record stable writes; the last record carries `b25 = 1` (the FC-flag
    /// candidate) so the dump shows a diagnostic.
    fn v1_fixture() -> OsgFile {
        let mut last = rec(1_900, counts(2, 1, 1, 1), 1_205, 0, None);
        last.max_combo = 4;
        last.b25 = 1;
        let mut miss = rec(1_500, counts(2, 1, 1, 1), 1_200, 0, None);
        miss.max_combo = 4;
        OsgFile {
            client_version: CLIENT,
            score_system: Some(OsgScoreSystem::V1),
            records: vec![
                rec(1_000, counts(1, 0, 0, 0), 320, 1, None),
                rec(1_250, counts(2, 1, 0, 0), 940, 3, None),
                rec(1_400, counts(2, 1, 1, 0), 1_140, 4, None),
                miss,
                last,
            ],
        }
    }

    fn v2_fixture() -> OsgFile {
        OsgFile {
            client_version: CLIENT,
            score_system: Some(OsgScoreSystem::V2),
            records: vec![
                rec(500, counts(1, 0, 0, 0), 150, 1, Some([150.0, 0.0])),
                rec(750, counts(2, 0, 0, 0), 300, 2, Some([300.0, 0.0])),
                rec(
                    900,
                    counts(2, 1, 0, 0),
                    537,
                    3,
                    Some([537.744_375_108_173_4, 0.0]),
                ),
            ],
        }
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn dump_of(osg: &OsgFile) -> OsgDump {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "fixture.osg", &encode_osg(osg));
        inspect(&path).unwrap()
    }

    #[test]
    fn inspect_missing_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = inspect(&dir.path().join("absent.osg")).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(err.args.contains_key("path"));
    }

    #[test]
    fn inspect_garbage_is_parse_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "garbage.osg", b"not an osg file at all");
        let err = inspect(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::ParseFailed);
        // The CLI prints `PARSE_FAILED: <variant> <details>` from this (spec 006 Behaviour).
        assert!(
            err.details
                .as_deref()
                .unwrap_or("")
                .starts_with("StrideMismatch")
        );
    }

    #[test]
    fn inspect_reports_header_and_deltas() {
        let dump = dump_of(&v1_fixture());
        assert_eq!(dump.header.record_count, 5);
        assert_eq!(dump.header.stride, Some(29));
        assert_eq!(dump.header.score_system, Some("v1"));
        assert_eq!(dump.header.file_size, 8 + 5 * 29);
        assert_eq!(dump.header.diagnostics, 1);
        assert_eq!(dump.diagnostics[0].code, "osg.nonzero_reserved");
        let chord = &dump.records[1];
        assert_eq!(
            (chord.dmax, chord.d300, chord.cmax, chord.c300),
            (1, 1, 2, 1)
        );
        assert!(dump.events[4].score_only);
        assert_eq!(dump.events[1].kinds, vec!["MAX", "300"]);
    }

    #[test]
    fn osg_dump_v1_table() {
        let dump = dump_of(&v1_fixture());
        let out = render_dump(&dump, DumpFormat::Table, DumpView::Records, None).unwrap();
        insta::assert_snapshot!("osg_dump_v1_table", out);
    }

    #[test]
    fn osg_dump_v2_json() {
        let dump = dump_of(&v2_fixture());
        let out = render_dump(&dump, DumpFormat::Json, DumpView::Records, None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed.get("events").is_none());
        insta::assert_snapshot!("osg_dump_v2_json", out);
    }

    #[test]
    fn osg_dump_events() {
        let dump = dump_of(&v1_fixture());
        let out = render_dump(&dump, DumpFormat::Table, DumpView::Events, None).unwrap();
        insta::assert_snapshot!("osg_dump_events", out);
    }

    #[test]
    fn csv_and_limit() {
        let dump = dump_of(&v2_fixture());
        let out = render_dump(&dump, DumpFormat::Csv, DumpView::Records, Some(2)).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("# client_version=20260924"));
        assert!(lines[1].starts_with("idx,t_ms,d300,") && lines[1].ends_with(",b28,f0,f1"));
        assert_eq!(lines.len(), 4, "{out}");
        assert_eq!(
            lines[3],
            "1,750,0,0,0,1,0,0,0,0,0,2,0,0,300,2,2,200,0,0,1,300,0"
        );
        let events = render_dump(&dump, DumpFormat::Json, DumpView::Events, Some(1)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&events).unwrap();
        assert_eq!(parsed["events"].as_array().unwrap().len(), 1);
        assert!(parsed.get("records").is_none());
    }

    #[test]
    fn empty_file_dumps_without_stride() {
        let dump = dump_of(&OsgFile {
            client_version: CLIENT,
            score_system: None,
            records: vec![],
        });
        assert_eq!((dump.header.stride, dump.header.score_system), (None, None));
        let out = render_dump(&dump, DumpFormat::Table, DumpView::Records, None).unwrap();
        assert!(out.starts_with("client_version=20260924 record_count=0 stride=- score_system=-"));
        assert_eq!(
            render_diagnostics(&dump),
            vec!["osg.empty_graph: 0 records"]
        );
    }
}

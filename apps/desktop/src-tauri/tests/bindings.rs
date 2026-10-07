#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 005 AC8: the committed `bindings.ts` is what the command list exports, byte for byte.

// The module makes nextest names `bindings::…`, so `nextest run bindings` selects them.
mod bindings {
    use std::path::Path;

    use serde::Serialize;
    use tauri::test::MockRuntime;
    use wolluf_desktop::bindings::{HEADER, committed_path, export_bindings, export_with};
    use wolluf_desktop::specta_builder;

    fn export_to_string(dir: &Path, name: &str) -> String {
        let path = dir.join(name);
        export_bindings(&path).unwrap();
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn committed_file_is_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = export_to_string(dir.path(), "bindings.ts");
        let committed = std::fs::read_to_string(committed_path()).unwrap_or_default();
        assert!(
            fresh == committed,
            "{} is stale: run `cargo xtask bindings` and commit it",
            committed_path().display()
        );
    }

    #[test]
    fn export_is_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let first = export_to_string(dir.path(), "a.ts");
        let second = export_to_string(dir.path(), "b.ts");
        assert_eq!(first, second);
        assert!(
            first.starts_with(HEADER),
            "{}",
            &first[..first.len().min(200)]
        );
        for name in [
            "setupStatus",
            "jobsStart",
            "appOpenLogsDir",
            "playersListAliases",
            "playersListProfiles",
            "playersSetProfileAliases",
            "playersDecideAlias",
            "playersCreateProfile",
            "playersSetDefault",
            "chartWindow",
            "ChartWindowDto",
            "chartAudio",
            "ChartAudioDto",
            "skinList",
            "SkinListDto",
            "skinGet",
            "SkinDto",
            "labelTaxonomy",
            "labelSample",
            "labelResolvePatterns",
            "labelReshape",
            "labelSubmit",
            "labelUndo",
            "labelPatternExamples",
            "PatternExampleDto",
            "labelStats",
            "AliasRowDto",
            "job-progress",
            "IpcError",
        ] {
            assert!(first.contains(name), "{name} missing from bindings");
        }
    }

    /// `number` loses precision above 2^53, so a 64-bit DTO field must break the export.
    #[derive(Serialize, specta::Type)]
    struct Wide {
        rows: u64,
    }

    #[test]
    fn bigint_field_fails_export() {
        let dir = tempfile::tempdir().unwrap();
        let builder = specta_builder::<MockRuntime>().typ::<Wide>();
        let err = export_with(&builder, &dir.path().join("wide.ts")).unwrap_err();
        assert!(err.to_string().contains("BigInt"), "{err}");
    }
}

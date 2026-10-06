#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 005 AC11: every core `ErrorCode` has an en and es fallback, so the UI never shows a raw key.

// The module makes nextest names `i18n_error_keys::…`, so `nextest run i18n_error_keys` selects them.
mod i18n_error_keys {
    use std::path::PathBuf;

    use wolluf_app::errors::keys;
    use wolluf_core::ErrorCode;

    const LOCALES: [&str; 2] = ["en", "es"];

    fn error_json(lang: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/src/shared/i18n/locales")
            .join(lang)
            .join("error.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    }

    #[test]
    fn every_error_code_has_en_and_es_fallback() {
        let mut missing = Vec::new();
        for lang in LOCALES {
            let json = error_json(lang);
            for code in ErrorCode::ALL {
                let value = &json["error"]["code"][code.as_str()];
                if !value.as_str().is_some_and(|s| !s.trim().is_empty()) {
                    missing.push(format!("{lang}: error.code.{}", code.as_str()));
                }
            }
        }
        assert!(missing.is_empty(), "missing fallbacks: {missing:?}");
    }

    /// ADR 0018: the Label screen shows these when a chart's audio cannot be served.
    #[test]
    fn chart_audio_keys_have_en_and_es_text() {
        let mut missing = Vec::new();
        for lang in LOCALES {
            let json = error_json(lang);
            for key in [keys::CHART_AUDIO_UNAVAILABLE, keys::CHART_AUDIO_TOO_LARGE] {
                let value = key.split('.').fold(&json, |node, part| &node[part]);
                if !value.as_str().is_some_and(|s| !s.trim().is_empty()) {
                    missing.push(format!("{lang}: {key}"));
                }
            }
        }
        assert!(missing.is_empty(), "missing keys: {missing:?}");
    }
}

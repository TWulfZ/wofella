#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 005 AC6: the commands answer over the real IPC path of the mock runtime.

// The module makes nextest names `commands_smoke::…`, so `nextest run commands_smoke` selects them.
mod commands_smoke {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{App, WebviewWindow, WebviewWindowBuilder};
    use wolluf_app::context::{AppContext, AppPaths};
    use wolluf_core::{FixedClock, UnixUs};
    use wolluf_desktop::{manage_context, specta_builder, with_plugins};

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);
    /// Crockford base32, 26 characters (003's `JobId`).
    const ULID_LEN: usize = 26;

    struct Harness {
        dir: tempfile::TempDir,
        ctx: Arc<AppContext>,
        _app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
    }

    impl Harness {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let paths = AppPaths::from_data_dir(dir.path().join("data"));
            let ctx = Arc::new(AppContext::open(paths, Arc::new(FixedClock::new(T0))).unwrap());
            let app = with_plugins(mock_builder())
                .invoke_handler(specta_builder().invoke_handler())
                .build(mock_context(noop_assets()))
                .unwrap();
            // The mock runtime never runs `setup` hooks, so the context is managed on the built app.
            manage_context(&app, ctx.clone());
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            Self {
                dir,
                ctx,
                _app: app,
                webview,
            }
        }

        fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_owned(),
            };
            get_ipc_response(&self.webview, request).map(|b| b.deserialize::<Value>().unwrap())
        }
    }

    #[test]
    fn setup_status_ok() {
        let h = Harness::new();
        let status = h.invoke("setup_status", json!({})).unwrap();
        assert_eq!(status["install"], Value::Null);
        assert_eq!(status["lastSync"], Value::Null);
        assert_eq!(
            status["dataDir"],
            json!(h.dir.path().join("data").to_string_lossy())
        );
        assert_eq!(status["appVersion"], json!(env!("CARGO_PKG_VERSION")));
        assert!(status["identityReady"].is_boolean(), "{status}");
    }

    #[test]
    fn setup_set_install_path_error_has_code() {
        let h = Harness::new();
        let missing = h.dir.path().join("no-osu-here");
        let err = h
            .invoke(
                "setup_set_install_path",
                json!({ "path": missing.to_string_lossy() }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("OSU_DIR_NOT_FOUND"), "{err}");
        assert_eq!(err["messageKey"], json!("error.code.OSU_DIR_NOT_FOUND"));
        assert_eq!(err["args"]["path"], json!(missing.to_string_lossy()));
        assert_eq!(err["retryable"], json!(false));
    }

    #[test]
    fn jobs_start_returns_string_id() {
        let h = Harness::new();
        // Registration skips validation, so a bare directory is enough for `start` to enqueue; the
        // job itself then fails in the background, which this test does not look at.
        let root = h.dir.path().join("osu");
        std::fs::create_dir_all(&root).unwrap();
        let install = h
            .ctx
            .runtime()
            .block_on(h.ctx.register_install(root, None))
            .unwrap();
        let id = h
            .invoke(
                "jobs_start",
                json!({ "request": { "kind": "sync_plays", "installId": install.0 } }),
            )
            .unwrap();
        let id = id.as_str().unwrap_or_else(|| panic!("not a string: {id}"));
        assert_eq!(id.len(), ULID_LEN, "{id}");

        assert!(h.invoke("jobs_list", json!({})).unwrap().is_array());
        // Cancelling the real job races its background failure, so the error path is checked on an
        // id that never existed.
        let err = h
            .invoke("jobs_cancel", json!({ "id": "01J0000000000000000000000Z" }))
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");
    }
}

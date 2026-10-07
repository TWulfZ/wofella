#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 005 AC7: `AppEvent`s reach the webview under their wire names with unchanged payloads.

// The module makes nextest names `event_bridge::…`, so `nextest run event_bridge` selects them.
mod event_bridge {
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::{Value, json};
    use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
    use tauri::{App, Listener};
    use tokio::sync::broadcast;
    use wolluf_app::events::{AppEvent, JobFinishedDto, JobProgressDto, SessionPlayAddedDto};
    use wolluf_app::jobs::dto::{JobId, JobKindDto, JobStageDto, JobStatusDto};
    use wolluf_desktop::events::{spawn_bridge, spawn_bridge_with};
    use wolluf_desktop::specta_builder;

    const WIRE_NAMES: [&str; 4] = [
        "job-progress",
        "job-finished",
        "data-changed",
        "session-play-added",
    ];
    const RECV_TIMEOUT: Duration = Duration::from_secs(5);

    struct Harness {
        _app: App<MockRuntime>,
        received: mpsc::Receiver<(String, Value)>,
    }

    /// Records every bridged event as `(wire name, payload)`, in arrival order.
    fn harness(
        rx: broadcast::Receiver<AppEvent>,
    ) -> (Harness, tauri::async_runtime::JoinHandle<()>) {
        harness_with(rx, None)
    }

    /// `attention` counts the window flashes the bridge asks for; `None` uses the real one.
    fn harness_with(
        rx: broadcast::Receiver<AppEvent>,
        attention: Option<mpsc::Sender<()>>,
    ) -> (Harness, tauri::async_runtime::JoinHandle<()>) {
        let specta = specta_builder::<MockRuntime>();
        let app = mock_builder()
            .invoke_handler(specta.invoke_handler())
            .build(mock_context(noop_assets()))
            .unwrap();
        specta.mount_events(&app);
        let (tx, received) = mpsc::channel();
        for name in WIRE_NAMES {
            let tx = tx.clone();
            app.listen_any(name, move |event| {
                let payload = serde_json::from_str(event.payload()).unwrap();
                tx.send((name.to_owned(), payload)).unwrap();
            });
        }
        let bridge = match attention {
            Some(flashes) => spawn_bridge_with(app.handle().clone(), rx, move |_| {
                flashes.send(()).unwrap();
            }),
            None => spawn_bridge(app.handle().clone(), rx),
        };
        (
            Harness {
                _app: app,
                received,
            },
            bridge,
        )
    }

    impl Harness {
        fn next(&self) -> (String, Value) {
            self.received.recv_timeout(RECV_TIMEOUT).unwrap()
        }
    }

    fn progress(done: u32) -> JobProgressDto {
        JobProgressDto {
            job_id: JobId("01JAAAAAAAAAAAAAAAAAAAAAAA".to_owned()),
            kind: JobKindDto::SyncPlays,
            stage: JobStageDto::Ingest,
            done,
            total: 4338,
            eta_ms: Some(1200),
        }
    }

    #[test]
    fn forwards_job_progress_payload() {
        let (tx, rx) = broadcast::channel(16);
        let (h, bridge) = harness(rx);
        tx.send(AppEvent::JobProgress(progress(7))).unwrap();
        let (name, payload) = h.next();
        assert_eq!(name, "job-progress");
        assert_eq!(payload, serde_json::to_value(progress(7)).unwrap());
        assert_eq!(payload["jobId"], json!("01JAAAAAAAAAAAAAAAAAAAAAAA"));
        drop(tx);
        // A closed bus ends the bridge instead of spinning.
        tauri::async_runtime::block_on(bridge).unwrap();
    }

    #[test]
    fn forwards_data_changed() {
        let (tx, rx) = broadcast::channel(16);
        let (h, _bridge) = harness(rx);
        tx.send(AppEvent::data_changed(&["players", "setup"]))
            .unwrap();
        assert_eq!(
            h.next(),
            (
                "data-changed".to_owned(),
                json!({ "domains": ["players", "setup"] })
            )
        );
    }

    #[test]
    fn forwards_job_finished() {
        let (tx, rx) = broadcast::channel(16);
        let (h, _bridge) = harness(rx);
        tx.send(AppEvent::JobFinished(JobFinishedDto {
            job_id: JobId("01JBBBBBBBBBBBBBBBBBBBBBBB".to_owned()),
            status: JobStatusDto::Ok,
            failed_items: 37,
        }))
        .unwrap();
        assert_eq!(
            h.next(),
            (
                "job-finished".to_owned(),
                json!({ "jobId": "01JBBBBBBBBBBBBBBBBBBBBBBB", "status": "ok", "failedItems": 37 })
            )
        );
    }

    #[test]
    fn lagged_receiver_emits_jobs_data_changed() {
        let (tx, rx) = broadcast::channel(1);
        // Two sends into a one-slot bus before the bridge reads: its first `recv` reports a lag.
        tx.send(AppEvent::JobProgress(progress(1))).unwrap();
        tx.send(AppEvent::JobProgress(progress(2))).unwrap();
        let (h, _bridge) = harness(rx);
        assert_eq!(
            h.next(),
            ("data-changed".to_owned(), json!({ "domains": ["jobs"] }))
        );
        let (name, payload) = h.next();
        assert_eq!(name, "job-progress");
        assert_eq!(payload["done"], json!(2));
    }

    #[test]
    fn forwards_session_play_added() {
        let (tx, rx) = broadcast::channel(16);
        let (h, _bridge) = harness(rx);
        tx.send(AppEvent::SessionPlayAdded(SessionPlayAddedDto {
            play_id: "ab".repeat(32),
            md5: "cd".repeat(16),
        }))
        .unwrap();
        assert_eq!(
            h.next(),
            (
                "session-play-added".to_owned(),
                json!({ "playId": "ab".repeat(32), "md5": "cd".repeat(16) })
            )
        );
    }

    /// The flash is a window call, never a webview event; the real call is a no-op without a
    /// main window.
    #[test]
    fn attention_request_flashes_the_window_without_a_webview_event() {
        let (tx, rx) = broadcast::channel(16);
        let (flashes_tx, flashes) = mpsc::channel();
        let (h, _bridge) = harness_with(rx, Some(flashes_tx));
        tx.send(AppEvent::AttentionRequested).unwrap();
        tx.send(AppEvent::data_changed(&["plays"])).unwrap();
        flashes.recv_timeout(RECV_TIMEOUT).unwrap();
        assert_eq!(
            h.next(),
            ("data-changed".to_owned(), json!({ "domains": ["plays"] }))
        );

        let (tx, rx) = broadcast::channel(16);
        let (h, _bridge) = harness(rx);
        tx.send(AppEvent::AttentionRequested).unwrap();
        tx.send(AppEvent::data_changed(&["plays"])).unwrap();
        assert_eq!(h.next().0, "data-changed");
    }
}

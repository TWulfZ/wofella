#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 005 AC6: the commands answer over the real IPC path of the mock runtime.

// The module makes nextest names `commands_smoke::…`, so `nextest run commands_smoke` selects them.
mod commands_smoke {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
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
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/dbs");
    /// Fixture osu!.db entry 1, a 7K map: `md5("wolluf-fixture:1")` at `folder-1/file-1`
    /// (fixtures/dbs/MANIFEST.toml).
    const FIXTURE_7K_MD5: &str = "643833896a402cef06fd6ee5c120211a";
    /// md5 of [`chart`]'s bytes. The shell may not depend on an md5 crate, so it is pinned; a
    /// stale value shows up as `NOT_FOUND`, because sync verifies the file against it.
    const CHART_MD5: &str = "241aa444ab16b2c707d1021d0f47ff99";
    const SEVEN_KEYS: u32 = 7;
    /// The fixture scores.db's one named player, made the session user so sync selects it.
    const FIXTURE_PLAYER: &str = "player-01";

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

    /// 32 taps 125 ms apart walking the columns, then one LN in column 3 at 4.0-4.5 s.
    fn chart() -> Vec<u8> {
        let x = |col: u32| (2 * col + 1) * 256 / SEVEN_KEYS;
        let mut text = format!(
            "osu file format v14\n\n[General]\nAudioFilename: audio.mp3\nMode: 3\n\n\
             [Metadata]\nTitle: t\nArtist: a\nCreator: c\nVersion: v\n\n\
             [Difficulty]\nHPDrainRate: 8\nCircleSize: {SEVEN_KEYS}\nOverallDifficulty: 8\n\n\
             [TimingPoints]\n0,500,4,1,0,100,1,0\n\n[HitObjects]\n"
        );
        for i in 0..32 {
            text.push_str(&format!(
                "{},192,{},1,0,0:0:0:0:\n",
                x(i % SEVEN_KEYS),
                i * 125
            ));
        }
        text.push_str(&format!("{},192,4000,128,0,4500:0:0:0:0:\n", x(3)));
        text.into_bytes()
    }

    /// The committed minimized install with fixture entry 1 re-pointed at [`chart`]: an md5 has
    /// a fixed length, so swapping it in place keeps osu!.db valid.
    fn install_with_chart(dir: &Path) -> PathBuf {
        let root = dir.join("osu!");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("osu!.exe"), b"MZ").unwrap();
        let fixtures = Path::new(FIXTURES);
        std::fs::copy(
            fixtures.join("scores_db/scores-20260924.min.db"),
            root.join("scores.db"),
        )
        .unwrap();
        let mut db = std::fs::read(fixtures.join("osu_db/osu-20260924.min.db")).unwrap();
        let at = db
            .windows(FIXTURE_7K_MD5.len())
            .position(|w| w == FIXTURE_7K_MD5.as_bytes())
            .unwrap();
        db[at..at + CHART_MD5.len()].copy_from_slice(CHART_MD5.as_bytes());
        std::fs::write(root.join("osu!.db"), db).unwrap();
        std::fs::write(
            root.join("osu!.fixture.cfg"),
            format!("Username = {FIXTURE_PLAYER}\n"),
        )
        .unwrap();
        let folder = root.join("Songs").join("folder-1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("file-1"), chart()).unwrap();
        root
    }

    /// Synced and indexed, so the chart is parsed and the self profile exists.
    fn synced() -> Harness {
        let h = Harness::new();
        let root = install_with_chart(h.dir.path());
        let rt = h.ctx.runtime();
        let install = rt.block_on(h.ctx.register_install(root, None)).unwrap();
        rt.block_on(h.ctx.plays().sync_and_wait(install)).unwrap();
        h
    }

    #[test]
    fn chart_window_returns_notes() {
        let h = synced();
        let window = h
            .invoke(
                "chart_window",
                json!({ "md5": CHART_MD5, "fromMs": 3_800, "toMs": 4_200, "layoutId": null }),
            )
            .unwrap();
        assert_eq!(
            window["notes"],
            json!([
                { "tMs": 3_875, "col": 3, "endMs": null },
                { "tMs": 4_000, "col": 3, "endMs": 4_500 },
            ]),
            "{window}"
        );
        assert_eq!(window["keymode"], json!(7));
        assert_eq!(window["layout"]["columns"].as_array().unwrap().len(), 7);
        assert_eq!(window["audioFilename"], json!("audio.mp3"));
        assert_eq!(window["chartSpan"], json!({ "firstMs": 0, "endMs": 4_500 }));

        let err = h
            .invoke(
                "chart_window",
                json!({ "md5": CHART_MD5, "fromMs": 1, "toMs": 1, "layoutId": null }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
    }

    #[test]
    fn chart_audio_reads_the_set_folder_only() {
        let h = synced();
        let err = h
            .invoke("chart_audio", json!({ "md5": CHART_MD5 }))
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");
        assert_eq!(err["messageKey"], json!("error.chart_audio_unavailable"));

        // Only a test temp dir is written; the chart's `AudioFilename` is `audio.mp3`.
        let set = h.dir.path().join("osu!").join("Songs").join("folder-1");
        std::fs::write(set.join("audio.mp3"), b"ID3").unwrap();
        let audio = h
            .invoke("chart_audio", json!({ "md5": CHART_MD5 }))
            .unwrap();
        assert_eq!(audio, json!({ "mime": "audio/mpeg", "base64": "SUQz" }));

        let err = h
            .invoke("chart_audio", json!({ "md5": "../folder-1/audio.mp3" }))
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
    }

    /// Signature plus an IHDR chunk: a synthetic header, never a real skin image.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend_from_slice(&13u32.to_be_bytes());
        out.extend_from_slice(b"IHDR");
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&[8, 6, 0, 0, 0]);
        out.extend_from_slice(&[0; 4]);
        out
    }

    #[test]
    fn skin_commands_serve_the_cfg_skin() {
        let h = synced();
        // Only a test temp dir is written.
        let root = h.dir.path().join("osu!");
        let skin = root.join("Skins").join("Test #1");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(
            skin.join("Skin.ini"),
            "[General]\r\nName: Test\r\nVersion: latest\r\n[Mania]\r\nKeys: 7\r\nHitPosition: 428\r\n",
        )
        .unwrap();
        std::fs::write(skin.join("mania-key1.PNG"), png(40, 100)).unwrap();
        std::fs::write(
            root.join("osu!.fixture.cfg"),
            format!("Username = {FIXTURE_PLAYER}\nSkin = Test #1\nManiaSpeed = 30\n"),
        )
        .unwrap();

        let mut list = h.invoke("skin_list", json!({})).unwrap();
        let mtime = list["skins"][0]
            .as_object_mut()
            .and_then(|skin| skin.remove("iniMtime"));
        assert!(mtime.as_ref().is_some_and(|m| m.is_string()), "{mtime:?}");
        assert_eq!(
            list,
            json!({
                "skins": [{ "folder": "Test #1", "name": "Test", "keymodes": [7] }],
                "current": "Test #1",
                "maniaSpeed": 30,
                "maniaSpeedBpmScale": null,
            })
        );

        let got = h
            .invoke("skin_get", json!({ "folder": "Test #1", "keymode": 7 }))
            .unwrap();
        assert_eq!(got["version"], json!(2.7), "{got}");
        assert_eq!(got["config"]["hitPosition"], json!(428.0));
        assert_eq!(got["config"]["noteBodyStyle"], json!("repeat_bottom"));
        let key0 = got["images"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["slot"] == json!("key.0"))
            .unwrap_or_else(|| panic!("key.0 unresolved: {got}"));
        let file = &got["files"][key0["file"].as_u64().unwrap() as usize];
        assert_eq!(file["mime"], json!("image/png"));
        assert_eq!((&file["width"], &file["height"]), (&json!(40), &json!(100)));
        assert!(
            file["base64"].as_str().unwrap().starts_with("iVBORw0KGgo"),
            "{file}"
        );

        let err = h
            .invoke(
                "skin_get",
                json!({ "folder": "../Skins/Test #1", "keymode": 7 }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");
        assert_eq!(err["messageKey"], json!("error.skin_unavailable"));
        let err = h
            .invoke("skin_get", json!({ "folder": "Test #1", "keymode": 0 }))
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
    }

    #[test]
    fn label_submit_then_undo_round_trips() {
        let h = synced();
        let taxonomy = h.invoke("label_taxonomy", json!({ "keymode": 7 })).unwrap();
        assert!(!taxonomy.as_array().unwrap().is_empty(), "{taxonomy}");
        let ids = json!(["regular.stream.jumpstream"]);
        let sampled = h
            .invoke(
                "label_sample",
                json!({ "req": {
                    "keymode": 7, "seed": "1", "round": 0, "windowMs": 2_000, "scale": null,
                    "levelMin": null, "levelMax": null, "exclude": [],
                } }),
            )
            .unwrap();
        assert_eq!(sampled["anchor"]["md5"], json!(CHART_MD5), "{sampled}");

        let anchor =
            json!({ "md5": CHART_MD5, "t0Ms": 0, "t1Ms": 2_000, "cols": [1, 2, 3, 4, 5, 6, 7] });
        let event = h
            .invoke(
                "label_submit",
                json!({ "req": {
                    "anchor": anchor, "patterns": ids, "noPattern": false, "mixed": false,
                    "unsure": false, "thumbPref": null,
                } }),
            )
            .unwrap();
        let id = event["id"].as_str().unwrap().to_owned();
        assert_eq!(id.len(), ULID_LEN, "{id}");
        assert_eq!(
            h.invoke("label_stats", json!({})).unwrap()["total"],
            json!(1)
        );

        h.invoke("label_undo", json!({ "eventId": id })).unwrap();
        assert_eq!(
            h.invoke("label_stats", json!({})).unwrap()["total"],
            json!(0)
        );
        let err = h
            .invoke("label_undo", json!({ "eventId": "not a ulid" }))
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
    }

    #[test]
    fn label_polish_commands_answer() {
        let h = synced();
        let anchor = json!({ "md5": CHART_MD5, "t0Ms": 0, "t1Ms": 2_000, "cols": [1, 2, 3] });
        let moved = h
            .invoke(
                "label_move_window",
                json!({ "req": { "anchor": anchor, "t0Ms": 4_000 } }),
            )
            .unwrap();
        assert_eq!(
            moved,
            json!({ "md5": CHART_MD5, "t0Ms": 2_501, "t1Ms": 4_501, "cols": [1, 2, 3, 4, 5, 6, 7] })
        );
        let timeline = h
            .invoke(
                "label_chart_timeline",
                json!({ "req": { "keymode": 7, "md5": CHART_MD5, "buckets": 2 } }),
            )
            .unwrap();
        assert_eq!(
            timeline,
            json!({ "firstMs": 0, "endMs": 4_501, "density": [19, 14], "labelled": [] })
        );
        let sampled = h
            .invoke(
                "label_sample",
                json!({ "req": {
                    "keymode": 7, "seed": "1", "round": 0, "windowMs": 2_000, "scale": null,
                    "levelMin": null, "levelMax": null, "exclude": [],
                } }),
            )
            .unwrap();
        assert!(sampled["creator"].is_string(), "{sampled}");
        assert!(
            sampled.as_object().unwrap().contains_key("stars"),
            "{sampled}"
        );
        assert_eq!(
            h.invoke("chart_background", json!({ "md5": CHART_MD5 }))
                .unwrap(),
            Value::Null,
            "the chart names no background"
        );

        let layouts = h
            .invoke("settings_hand_layouts", json!({ "keymode": 7 }))
            .unwrap();
        assert_eq!(layouts.as_array().unwrap().len(), 5, "{layouts}");
        assert_eq!(layouts[0]["id"], json!("k7.313_right_thumb"));
        assert_eq!(
            layouts[0]["columns"][3],
            json!({ "hand": "right", "finger": "thumb" })
        );
        let get = |h: &Harness| {
            h.invoke("settings_get_hand_layout", json!({ "keymode": 7 }))
                .unwrap()
        };
        assert_eq!(get(&h), json!("k7.313_right_thumb"));
        h.invoke(
            "settings_set_hand_layout",
            json!({ "keymode": 7, "layoutId": "k7.313_left_thumb" }),
        )
        .unwrap();
        assert_eq!(get(&h), json!("k7.313_left_thumb"));
        let err = h
            .invoke(
                "settings_set_hand_layout",
                json!({ "keymode": 7, "layoutId": "nope" }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
        let examples = h
            .invoke(
                "label_pattern_examples",
                json!({ "keymode": 7, "layoutId": null }),
            )
            .unwrap();
        assert_eq!(
            examples[0]["window"]["layout"]["id"],
            json!("k7.313_left_thumb")
        );
    }

    #[test]
    fn label_player_commands_answer() {
        let h = synced();
        let anchor = json!({ "md5": CHART_MD5, "t0Ms": 0, "t1Ms": 2_000, "cols": [1, 2, 3] });
        let resize = |t0: i32, t1: i32| {
            h.invoke(
                "label_resize_window",
                json!({ "req": { "anchor": anchor, "t0Ms": t0, "t1Ms": t1 } }),
            )
            .unwrap()
        };
        assert_eq!(
            resize(0, 3_000),
            json!({ "md5": CHART_MD5, "t0Ms": 0, "t1Ms": 3_000, "cols": [1, 2, 3, 4, 5, 6, 7] })
        );
        assert_eq!(resize(0, 90_000)["t1Ms"], json!(4_501));

        let details = h
            .invoke("chart_details", json!({ "md5": CHART_MD5 }))
            .unwrap();
        assert_eq!(details["md5"], json!(CHART_MD5));
        assert_eq!(
            (
                &details["nNotes"],
                &details["nLn"],
                &details["lengthMs"],
                &details["bpmMin"],
                &details["bpmMax"],
            ),
            (
                &json!(33),
                &json!(1),
                &json!(4_500),
                &json!(120.0),
                &json!(120.0)
            ),
            "{details}"
        );
        assert!(details["tags"].is_array(), "{details}");
        let err = h
            .invoke("chart_details", json!({ "md5": "0".repeat(32) }))
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");
    }

    #[test]
    fn session_commands_answer() {
        let h = synced();
        // Every fixture play predates the session, so nothing is pending.
        let plays = h.invoke("session_plays", json!({ "keymode": 7 })).unwrap();
        assert_eq!(plays["plays"], json!([]), "{plays}");
        assert!(
            plays["startedAt"].as_str().unwrap().ends_with('Z'),
            "{plays}"
        );

        let event = h
            .invoke(
                "session_label_submit",
                json!({ "req": {
                    "keymode": 7, "md5": CHART_MD5, "playId": null,
                    "pattern": "regular.stream.jumpstream",
                } }),
            )
            .unwrap();
        let id = event["id"].as_str().unwrap().to_owned();
        assert_eq!(id.len(), ULID_LEN, "{id}");
        let progress = |h: &Harness| {
            h.invoke(
                "label_progress",
                json!({ "keymode": 7, "utcOffsetMin": -300 }),
            )
            .unwrap()
        };
        let p = progress(&h);
        assert_eq!(
            (&p["sessionLabels"], &p["goldTotal"]),
            (&json!(1), &json!(0)),
            "{p}"
        );
        assert_eq!(p["perDay"].as_array().unwrap().len(), 30, "{p}");
        // ADR 0020: a session label never reaches the gold stats.
        assert_eq!(
            h.invoke("label_stats", json!({})).unwrap()["total"],
            json!(0)
        );

        let err = h
            .invoke("label_undo", json!({ "eventId": id }))
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");
        h.invoke("session_label_undo", json!({ "eventId": id }))
            .unwrap();
        assert_eq!(progress(&h)["sessionLabels"], json!(0));
        let err = h
            .invoke(
                "session_label_submit",
                json!({ "req": { "keymode": 7, "md5": CHART_MD5, "playId": null, "pattern": "x" } }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");

        assert_eq!(
            h.invoke("settings_get_session_notify", json!({})).unwrap(),
            json!(false)
        );
        h.invoke("settings_set_session_notify", json!({ "on": true }))
            .unwrap();
        assert_eq!(
            h.invoke("settings_get_session_notify", json!({})).unwrap(),
            json!(true)
        );
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

    /// Spec 004 AC13: the six players commands answer over IPC. Data-dependent behaviour is
    /// covered by `PlayersService`'s tests; an empty ledger is enough to prove the wiring.
    #[test]
    fn players_commands_answer() {
        let h = Harness::new();
        let list = h.invoke("players_list_aliases", json!({})).unwrap();
        assert_eq!(list["aliases"], json!([]), "{list}");
        assert_eq!(list["wizardNeeded"], json!(false));
        assert_eq!(list["cfgUsernameAvailable"], json!(false));
        assert!(list["selectionVersion"].is_number(), "{list}");

        let decided = h
            .invoke(
                "players_decide_alias",
                json!({ "input": { "decisions": [], "completesWizard": false } }),
            )
            .unwrap();
        assert_eq!(decided["aliases"], json!([]), "{decided}");

        // The decide call bootstrapped the self profile; with no aliases it has no scopes.
        let profiles = h
            .invoke("players_list_profiles", json!({ "keymode": 7 }))
            .unwrap();
        assert_eq!(
            profiles,
            json!([{
                "ref": { "kind": "profile", "id": 1 },
                "profileKind": "self",
                "label": "Me",
                "isDefault": true,
                "mergeMode": "merged",
                "aliasIds": [],
                "scopes": [],
            }, {
                "ref": { "kind": "all_players" },
                "profileKind": "all_players",
                "label": "",
                "isDefault": false,
                "mergeMode": "merged",
                "aliasIds": [],
                "scopes": [],
            }])
        );
    }

    #[test]
    fn players_errors_have_codes() {
        let h = Harness::new();
        let err = h
            .invoke("players_list_profiles", json!({ "keymode": 0 }))
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
        assert_eq!(err["messageKey"], json!("players.error.invalid_keymode"));

        let err = h
            .invoke("players_set_default", json!({ "profileId": 99 }))
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");

        let err = h
            .invoke(
                "players_create_profile",
                json!({ "input": { "label": "  ", "aliasIds": [1] } }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");

        let err = h
            .invoke(
                "players_set_profile_aliases",
                json!({ "input": { "profileId": 99, "aliasIds": [], "mergeMode": "separate" } }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("NOT_FOUND"), "{err}");

        let err = h
            .invoke(
                "players_decide_alias",
                json!({ "input": {
                    "decisions": [
                        { "aliasId": 1, "decision": "me" },
                        { "aliasId": 1, "decision": null },
                    ],
                    "completesWizard": true,
                } }),
            )
            .unwrap_err();
        assert_eq!(err["code"], json!("INVALID_INPUT"), "{err}");
        assert_eq!(err["messageKey"], json!("players.error.duplicate_alias"));
    }
}

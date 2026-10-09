#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::PathBuf;

use common::{Env, osu_7k};

/// 16-bit mono PCM WAV of silence: the copy's audio only has to decode.
fn wav(seconds: u32) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let data_len = RATE * seconds * 2;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    for v in [
        16u32.to_le_bytes().to_vec(),
        1u16.to_le_bytes().to_vec(),
        1u16.to_le_bytes().to_vec(),
    ] {
        out.extend_from_slice(&v);
    }
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(out.len() + data_len as usize, 0);
    out
}

/// A synced env whose chart names `audio.wav`, which sits in its set folder.
fn synced() -> (Env, String, PathBuf) {
    let taps: Vec<(u8, i32)> = (0..8)
        .map(|i| (i % 7, 1_000 + i32::from(i) * 250))
        .collect();
    let osu = String::from_utf8(osu_7k(&taps, &[]))
        .unwrap()
        .replace("audio.mp3", "audio.wav");
    let env = Env::new();
    let (root, md5) = env.install_with_chart(osu.as_bytes());
    let folder = root.join("Songs").join("folder-1");
    std::fs::write(folder.join("audio.wav"), wav(2)).unwrap();
    env.set_install(&root);
    env.json(&["sync"]);
    (env, md5, folder)
}

#[test]
fn plan_writes_nothing_and_create_writes_both_files_once() {
    let (env, md5, folder) = synced();
    let plan = env.json(&["rate-copy", "plan", &md5, "--rate", "1.15"]);
    assert_eq!(plan["refusal"], serde_json::Value::Null, "{plan}");
    assert_eq!(plan["audioFilename"], "audio 1.15x.ogg", "{plan}");
    let osu_name = plan["osuFilename"].as_str().unwrap().to_owned();
    assert_eq!(osu_name, "a - t (c) [v 1.15x (138bpm)].osu");
    assert!(!folder.join(&osu_name).exists(), "plan writes nothing");

    let out = env
        .wolluf()
        .args(["rate-copy", "create", &md5, "--rate", "1.15", "--yes"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("press F5 in osu! song select"), "{stdout}");
    let osu_path = folder.join(&osu_name);
    let ogg_path = folder.join("audio 1.15x.ogg");
    assert!(stdout.contains(&osu_path.display().to_string()), "{stdout}");
    assert!(stdout.contains(&ogg_path.display().to_string()), "{stdout}");
    let osu = std::fs::read(&osu_path).unwrap();
    let ogg = std::fs::read(&ogg_path).unwrap();
    assert!(ogg.starts_with(b"OggS"));

    // A second create renders nothing and replaces nothing.
    let again = env.json(&["rate-copy", "create", &md5, "--rate", "1.15", "--yes"]);
    let s = &again["job"]["summary"]["counters"];
    assert_eq!(again["job"]["kind"], "rate_copy", "{again}");
    assert_eq!(s["osuWritten"], false, "{again}");
    assert_eq!(s["audioReused"], true, "{again}");
    assert_eq!(std::fs::read(&osu_path).unwrap(), osu);
    assert_eq!(std::fs::read(&ogg_path).unwrap(), ogg);
}

#[test]
fn create_refuses_a_refused_plan_with_a_usage_exit() {
    let (env, md5, folder) = synced();
    let out = env
        .wolluf()
        .args(["rate-copy", "create", &md5, "--rate", "1.0", "--yes"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("rate_copy.error.refused {refusal=identity_rate}"),
        "{stderr}"
    );
    let names: Vec<_> = std::fs::read_dir(&folder).unwrap().collect();
    assert_eq!(names.len(), 2, "only the chart and its audio");
}

#[test]
fn nc_flag_plans_and_creates_the_pitch_shifted_copy() {
    let (env, md5, folder) = synced();
    let plan = env.json(&["rate-copy", "plan", &md5, "--rate", "1.15", "--nc"]);
    assert_eq!(plan["refusal"], serde_json::Value::Null, "{plan}");
    assert_eq!(plan["nightcore"], true, "{plan}");
    assert_eq!(plan["audioFilename"], "audio 1.15x nc.ogg", "{plan}");
    assert_eq!(plan["osuFilename"], "a - t (c) [v 1.15x NC (138bpm)].osu");

    let text = env
        .wolluf()
        .args(["rate-copy", "plan", &md5, "--rate", "1.15", "--nc"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&text.stdout).contains("NC (pitch follows the rate)"),
        "{}",
        String::from_utf8_lossy(&text.stdout)
    );

    let created = env.json(&[
        "rate-copy",
        "create",
        &md5,
        "--rate",
        "1.15",
        "--nc",
        "--yes",
    ]);
    assert_eq!(created["job"]["status"], "ok", "{created}");
    assert!(folder.join("audio 1.15x nc.ogg").is_file());
    assert!(folder.join("a - t (c) [v 1.15x NC (138bpm)].osu").is_file());
    assert!(!folder.join("audio 1.15x.ogg").exists());
}

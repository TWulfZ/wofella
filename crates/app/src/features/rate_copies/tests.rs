//! Plan, confirm and the `RateCopy` job over a temp install: every file lives in a tempdir.

use std::path::PathBuf;
use std::time::Duration;

use wolluf_audio::{AudioParams, decode};
use wolluf_core::ErrorCode;

use super::dto::{AUDIO_TARGET_UNUSABLE_KEY, REFUSED_KEY, RateCopyPlanDto, refusal};
use super::testkit::{OsuChart, wav};
use crate::events::AppEvent;
use crate::export::{self, keys as export_keys};
use crate::features::library::testkit::{Map, install};
use crate::features::plays::testkit::Fixture;
use crate::jobs::dto::{
    JobDto, JobId, JobKindDto, JobStatusDto, JobSummaryDto, RATE_COPY_NEXT_STEP, RateCopySummaryDto,
};

const WAIT: Duration = Duration::from_secs(120);
const AUDIO_SECONDS: f64 = 2.0;

struct Set {
    f: Fixture,
    map: Map,
}

impl Set {
    async fn new(chart: &OsuChart<'_>, extra: &[(&str, Vec<u8>)]) -> Self {
        Self::with_map(Map::new(chart.title, chart.keys, chart.bytes()), extra).await
    }

    async fn with_map(map: Map, extra: &[(&str, Vec<u8>)]) -> Self {
        let mut fake = install(std::slice::from_ref(&map), &[]);
        for (name, bytes) in extra {
            fake = fake.song(format!("{}/{name}", map.folder), bytes.clone());
        }
        let f = Fixture::new(&fake).await;
        f.sync().await;
        Self { f, map }
    }

    fn folder(&self) -> PathBuf {
        std::fs::canonicalize(self.f.root.join("Songs").join(&self.map.folder)).unwrap()
    }

    async fn plan(&self, rate_milli: u16) -> RateCopyPlanDto {
        self.plan_with(rate_milli, false).await
    }

    async fn plan_nc(&self, rate_milli: u16) -> RateCopyPlanDto {
        self.plan_with(rate_milli, true).await
    }

    async fn plan_with(&self, rate_milli: u16, nightcore: bool) -> RateCopyPlanDto {
        self.f
            .ctx
            .rate_copies()
            .plan(&self.map.md5, rate_milli, nightcore)
            .await
            .unwrap()
    }

    /// Confirms and waits for the job and everything it chains.
    async fn confirm(&self, preview_id: &str) -> (JobDto, Vec<AppEvent>) {
        let mut rx = self.f.ctx.subscribe();
        let id = self.f.ctx.rate_copies().confirm(preview_id).await.unwrap();
        let mut events = tokio::time::timeout(WAIT, async {
            let mut seen = Vec::new();
            loop {
                let event = rx.recv().await.unwrap();
                let done = matches!(&event, AppEvent::JobFinished(f) if f.job_id == id);
                seen.push(event);
                if done {
                    return seen;
                }
            }
        })
        .await
        .expect("rate copy finished in time");
        self.f.ctx.jobs().wait_idle().await;
        // `DataChanged` follows `JobFinished`.
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        (job(&self.f, &id).await, events)
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.folder())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

async fn job(f: &Fixture, id: &JobId) -> JobDto {
    f.ctx
        .jobs()
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .find(|j| &j.id == id)
        .unwrap()
}

fn summary(job: &JobDto) -> RateCopySummaryDto {
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    match &job.summary {
        Some(JobSummaryDto::RateCopy(s)) => s.clone(),
        other => panic!("unexpected summary {other:?}"),
    }
}

fn wav_audio() -> (&'static str, Vec<u8>) {
    ("audio.wav", wav(AUDIO_SECONDS))
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_names_the_copy_and_writes_nothing() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let before = set.files();
    let plan = set.plan(1150).await;
    assert!(!plan.preview_id.is_empty());
    assert_eq!(plan.refusal, None);
    assert_eq!(plan.md5, set.map.md5);
    assert_eq!(plan.rate_milli, 1150);
    assert!(!plan.nightcore);
    assert_eq!(PathBuf::from(&plan.folder), set.folder());
    assert_eq!(plan.version, "Normal 1.15x (138bpm)");
    assert_eq!(
        plan.osu_filename,
        "wolluf - T (wolluf) [Normal 1.15x (138bpm)].osu"
    );
    assert_eq!(plan.audio_filename, "audio 1.15x.ogg");
    assert!(!plan.audio_exists && !plan.osu_exists);
    assert_eq!(set.files(), before, "a plan writes nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_refusals_carry_stable_ids_and_no_preview() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    for (rate, id) in [
        (1000, refusal::IDENTITY_RATE),
        (2500, refusal::RATE_OUT_OF_RANGE),
    ] {
        let plan = set.plan(rate).await;
        assert_eq!(plan.refusal.as_deref(), Some(id));
        assert_eq!(plan.preview_id, "", "a refused plan cannot be confirmed");
    }

    let ln = Set::new(&OsuChart::ln_heavy("L"), &[wav_audio()]).await;
    assert_eq!(
        ln.plan(1100).await.refusal.as_deref(),
        Some(refusal::LN_HEAVY)
    );

    let osb = (
        "wolluf - T (wolluf).osb",
        b"[Events]\r\n//Storyboard Sound Samples\r\nSample,500,0,\"clap.wav\",80\r\n".to_vec(),
    );
    let keysounded = Set::new(&OsuChart::k7("T"), &[wav_audio(), osb]).await;
    let plan = keysounded.plan(1100).await;
    assert_eq!(plan.refusal.as_deref(), Some(refusal::KEYSOUNDED));
    assert_eq!(plan.preview_id, "");

    let silent = Set::new(&OsuChart::k7("T"), &[]).await;
    assert_eq!(
        silent.plan(1100).await.refusal.as_deref(),
        Some(refusal::AUDIO_MISSING)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_refuses_a_copy_whose_rounding_collides() {
    let chart = OsuChart {
        taps: vec![(0, 1_000), (0, 1_002), (1, 1_500)],
        ..OsuChart::k7("C")
    };
    let set = Set::new(&chart, &[wav_audio()]).await;
    assert_eq!(
        set.plan(2000).await.refusal.as_deref(),
        Some(refusal::SAME_COLUMN_COLLISION)
    );
    assert_eq!(set.plan(1100).await.refusal, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_errors_on_unknown_or_bad_charts() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let svc = set.f.ctx.rate_copies();
    assert_eq!(
        svc.plan("nope", 1100, false).await.unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        svc.plan(&"0".repeat(32), 1100, false)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_rejects_a_catalog_folder_outside_songs() {
    let chart = OsuChart::k7("T");
    let map = Map::new(chart.title, chart.keys, chart.bytes()).named("../escape", "Normal");
    let set = Set::with_map(map, &[]).await;
    let err = set
        .f
        .ctx
        .rate_copies()
        .plan(&set.map.md5, 1100, false)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput, "{err:?}");
    assert_eq!(err.message_key, export_keys::FOLDER_OUTSIDE_SONGS);
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_needs_a_recorded_preview() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let svc = set.f.ctx.rate_copies();
    for id in ["", "01JZZZZZZZZZZZZZZZZZZZZZZZ"] {
        let err = svc.confirm(id).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound, "{id:?}");
        assert_eq!(err.message_key, export_keys::PREVIEW_UNKNOWN);
    }
    let plan = set.plan(1100).await;
    set.confirm(&plan.preview_id).await;
    assert_eq!(
        svc.confirm(&plan.preview_id).await.unwrap_err().code,
        ErrorCode::NotFound,
        "a preview is confirmed once"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_job_writes_both_files_and_never_overwrites() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let plan = set.plan(1250).await;
    let (job, events) = set.confirm(&plan.preview_id).await;
    let s = summary(&job);
    assert_eq!(job.kind, JobKindDto::RateCopy);
    assert_eq!(
        (s.osu_written, s.audio_written, s.audio_reused),
        (true, true, false)
    );
    assert_eq!(s.next_step, RATE_COPY_NEXT_STEP);
    assert_eq!(PathBuf::from(&s.folder), set.folder());
    assert!(
        events.iter().any(
            |e| matches!(e, AppEvent::DataChanged(d) if d.domains.iter().any(|x| x == "library"))
        ),
        "library changed"
    );
    let jobs = set.f.ctx.jobs().list(None).await.unwrap();
    let index_after = jobs
        .iter()
        .take_while(|j| j.id != job.id)
        .any(|j| j.kind == JobKindDto::IndexLibrary);
    assert!(index_after, "IndexLibrary is chained");

    let osu_path = set.folder().join(&plan.osu_filename);
    let ogg_path = set.folder().join(&plan.audio_filename);
    let osu = std::fs::read_to_string(&osu_path).unwrap();
    assert!(osu.contains("Version:Normal 1.25x (150bpm)"), "{osu}");
    assert!(osu.contains("AudioFilename: audio 1.25x.ogg"), "{osu}");
    assert!(osu.contains("800,1,0,0:0:0:0:"), "1000 ms / 1.25: {osu}");
    let ogg = std::fs::read(&ogg_path).unwrap();
    let pcm = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    let seconds = pcm.samples.len() as f64 / f64::from(pcm.sample_rate) / f64::from(pcm.channels);
    assert!(
        (seconds - AUDIO_SECONDS / 1.25).abs() < 0.05,
        "stretched length {seconds}"
    );

    // Same plan again: both files exist, nothing is rendered or replaced.
    let before = (
        std::fs::read(&osu_path).unwrap(),
        std::fs::read(&ogg_path).unwrap(),
    );
    let again = set.plan(1250).await;
    assert!(again.audio_exists && again.osu_exists);
    let s = summary(&set.confirm(&again.preview_id).await.0);
    assert_eq!(
        (s.osu_written, s.audio_written, s.audio_reused),
        (false, false, true)
    );
    assert_eq!(
        (
            std::fs::read(&osu_path).unwrap(),
            std::fs::read(&ogg_path).unwrap()
        ),
        before
    );

    // A deleted .osu is written again next to the audio, which is reused.
    std::fs::remove_file(&osu_path).unwrap();
    let third = set.plan(1250).await;
    assert!(third.audio_exists && !third.osu_exists);
    let s = summary(&set.confirm(&third.preview_id).await.0);
    assert_eq!(
        (s.osu_written, s.audio_written, s.audio_reused),
        (true, false, true)
    );
    assert_eq!(std::fs::read(&ogg_path).unwrap(), before.1);
}

fn failed(job: &JobDto) -> String {
    assert_eq!(job.status, JobStatusDto::Failed, "{job:?}");
    job.error.as_ref().unwrap().message_key.clone()
}

fn no_temp_files(set: &Set) {
    let temps: Vec<String> = set
        .files()
        .into_iter()
        .filter(|n| n.starts_with(export::TEMP_PREFIX))
        .collect();
    assert!(temps.is_empty(), "temp files left: {temps:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_refuses_audio_names_that_are_not_one_plain_file() {
    for audio in ["../x.mp3", "sub/a.mp3", "a:b.mp3"] {
        let chart = OsuChart {
            audio,
            ..OsuChart::k7("T")
        };
        let set = Set::new(&chart, &[wav_audio()]).await;
        let before = set.files();
        let plan = set.plan(1150).await;
        assert_eq!(
            plan.refusal.as_deref(),
            Some(refusal::UNSAFE_NAME),
            "{audio}"
        );
        assert_eq!(plan.preview_id, "", "{audio}");
        assert_eq!(set.files(), before, "{audio}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_refuses_a_copy_name_longer_than_windows_allows() {
    // Over 255 bytes for the copy's name while the source's still fits.
    let title = "t".repeat(220);
    let chart = OsuChart::k7(&title);
    let map = Map::new(chart.title, chart.keys, chart.bytes()).named("100 long", "Normal");
    let set = Set::with_map(map, &[wav_audio()]).await;
    let plan = set.plan(1150).await;
    assert_eq!(plan.refusal.as_deref(), Some(refusal::UNSAFE_NAME));
    assert_eq!(plan.preview_id, "");

    // Each name fits; folder and name together pass Windows' MAX_PATH.
    let title = "t".repeat(200);
    let set = Set::new(&OsuChart::k7(&title), &[wav_audio()]).await;
    let plan = set.plan(1150).await;
    assert_eq!(plan.refusal.as_deref(), Some(refusal::UNSAFE_NAME));
}

#[tokio::test(flavor = "multi_thread")]
async fn plan_refuses_storyboard_samples_behind_variables() {
    let osb = (
        "wolluf - T (wolluf).osb",
        b"[Variables]\r\n$s=Sample\r\n[Events]\r\n$s,500,0,\"clap.wav\",80\r\n".to_vec(),
    );
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio(), osb]).await;
    assert_eq!(
        set.plan(1100).await.refusal.as_deref(),
        Some(refusal::KEYSOUNDED)
    );
    let chart = OsuChart {
        variables: &["$s=Sample"],
        events: &["$s,1000,0,\"kick.wav\",70"],
        ..OsuChart::k7("T")
    };
    let set = Set::new(&chart, &[wav_audio()]).await;
    assert_eq!(
        set.plan(1100).await.refusal.as_deref(),
        Some(refusal::KEYSOUNDED)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_osu_naming_other_audio_refuses_the_plan() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let name = set.plan(1150).await.osu_filename;
    std::fs::write(
        set.folder().join(&name),
        b"osu file format v14\r\n[General]\r\nAudioFilename: audio.wav\r\n",
    )
    .unwrap();
    let before = set.files();
    let plan = set.plan(1150).await;
    assert_eq!(plan.refusal.as_deref(), Some(refusal::ALREADY_EXISTS));
    assert_eq!(plan.preview_id, "");
    assert_eq!(set.files(), before);

    std::fs::remove_file(set.folder().join(&name)).unwrap();
    std::fs::create_dir(set.folder().join(&name)).unwrap();
    let plan = set.plan(1150).await;
    assert_eq!(plan.refusal.as_deref(), Some(refusal::ALREADY_EXISTS));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_osu_naming_the_copy_audio_gets_its_audio_rendered() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let first = set.plan(1150).await;
    std::fs::write(
        set.folder().join(&first.osu_filename),
        b"osu file format v14\r\n[General]\r\nAudioFilename: audio 1.15x.ogg\r\n",
    )
    .unwrap();
    let plan = set.plan(1150).await;
    assert_eq!(plan.refusal, None);
    assert!(plan.osu_exists && !plan.audio_exists);
    let s = summary(&set.confirm(&plan.preview_id).await.0);
    assert_eq!(
        (s.osu_written, s.audio_written, s.audio_reused),
        (false, true, false)
    );
}

/// A folder at the `.osu` name is caught before rendering; the rollback after a published audio
/// file is covered in `job::tests`.
#[tokio::test(flavor = "multi_thread")]
async fn an_osu_name_taken_after_the_plan_fails_before_any_audio() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let plan = set.plan(1250).await;
    std::fs::create_dir(set.folder().join(&plan.osu_filename)).unwrap();
    let before = set.files();
    let (job, _) = set.confirm(&plan.preview_id).await;
    assert_eq!(failed(&job), REFUSED_KEY);
    assert!(!set.folder().join(&plan.audio_filename).exists());
    assert_eq!(set.files(), before);
    no_temp_files(&set);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_audio_target_that_is_not_a_playable_file_fails_the_copy() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let audio = set.plan(1250).await.audio_filename;
    let target = set.folder().join(&audio);
    std::fs::create_dir(&target).unwrap();
    let plan = set.plan(1250).await;
    let (job, _) = set.confirm(&plan.preview_id).await;
    assert_eq!(failed(&job), AUDIO_TARGET_UNUSABLE_KEY);
    assert!(!set.folder().join(&plan.osu_filename).exists());

    std::fs::remove_dir(&target).unwrap();
    std::fs::write(&target, b"").unwrap();
    let plan = set.plan(1250).await;
    let (job, _) = set.confirm(&plan.preview_id).await;
    assert_eq!(failed(&job), AUDIO_TARGET_UNUSABLE_KEY);
    assert!(!set.folder().join(&plan.osu_filename).exists());
    assert!(std::fs::read(&target).unwrap().is_empty());
    no_temp_files(&set);
}

fn stretched_seconds(ogg: &[u8]) -> f64 {
    let pcm = decode(ogg, "ogg", &AudioParams::default()).unwrap();
    pcm.samples.len() as f64 / f64::from(pcm.sample_rate) / f64::from(pcm.channels)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_nightcore_plan_names_its_own_audio_and_version() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let before = set.files();
    let plan = set.plan_nc(1150).await;
    assert_eq!(plan.refusal, None);
    assert!(plan.nightcore);
    assert!(!plan.preview_id.is_empty());
    assert_eq!(plan.version, "Normal 1.15x NC (138bpm)");
    assert_eq!(
        plan.osu_filename,
        "wolluf - T (wolluf) [Normal 1.15x NC (138bpm)].osu"
    );
    assert_eq!(plan.audio_filename, "audio 1.15x nc.ogg");
    assert!(!plan.audio_exists && !plan.osu_exists);
    assert_eq!(set.files(), before, "a plan writes nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn pitch_kept_and_nightcore_copies_of_one_rate_coexist() {
    let set = Set::new(&OsuChart::k7("T"), &[wav_audio()]).await;
    let before = set.files();
    let dt = set.plan(1250).await;
    summary(&set.confirm(&dt.preview_id).await.0);
    let dt_files = (
        std::fs::read(set.folder().join(&dt.osu_filename)).unwrap(),
        std::fs::read(set.folder().join(&dt.audio_filename)).unwrap(),
    );

    let nc = set.plan_nc(1250).await;
    assert_eq!(nc.refusal, None);
    assert!(
        !nc.audio_exists && !nc.osu_exists,
        "the DT copy is not reused"
    );
    let (job, _) = set.confirm(&nc.preview_id).await;
    let s = summary(&job);
    assert_eq!(
        (s.osu_written, s.audio_written, s.audio_reused),
        (true, true, false)
    );
    assert_eq!(s.audio_filename, "audio 1.25x nc.ogg");

    let osu = std::fs::read_to_string(set.folder().join(&nc.osu_filename)).unwrap();
    assert!(osu.contains("AudioFilename: audio 1.25x nc.ogg"), "{osu}");
    assert!(osu.contains("Version:Normal 1.25x NC (150bpm)"), "{osu}");
    let nc_ogg = std::fs::read(set.folder().join(&nc.audio_filename)).unwrap();
    assert!(
        (stretched_seconds(&nc_ogg) - AUDIO_SECONDS / 1.25).abs() < 0.05,
        "stretched length"
    );
    assert_ne!(
        nc_ogg, dt_files.1,
        "the NC audio is rendered with its pitch"
    );

    let mut expected = before;
    expected.extend([
        dt.audio_filename.clone(),
        dt.osu_filename.clone(),
        nc.audio_filename.clone(),
        nc.osu_filename.clone(),
    ]);
    expected.sort();
    assert_eq!(set.files(), expected);
    assert_eq!(
        (
            std::fs::read(set.folder().join(&dt.osu_filename)).unwrap(),
            std::fs::read(set.folder().join(&dt.audio_filename)).unwrap(),
        ),
        dt_files,
        "the DT copy is untouched"
    );
}

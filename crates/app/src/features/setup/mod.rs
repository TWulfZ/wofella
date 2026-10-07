//! Setup: find, validate and register the osu! install, and report readiness (spec 005).
//! Discovery and validation are 002's `source_osu::install`; this slice maps them 1:1.

pub mod dto;

use std::path::Path;

use wolluf_source_osu::SourceError;
use wolluf_source_osu::install::{DetectEnv, InstallInfo, detect, validate_install};
use wolluf_store::repo::ledger::GameInstall;
use wolluf_store::time::format_rfc3339_ms;

use crate::context::{AppContext, blocking_join_error};
use crate::errors::AppError;
use crate::events::AppEvent;
use crate::jobs::JobKindDto;
use dto::{InstallCandidateDto, InstallDto, SetupStatusDto};

const DOMAIN_SETUP: &str = "setup";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Spec 005: a valid install without scores.db is fine (no plays yet) but still reported.
const MISSING_SCORES_DB: &str = "scores.db";

pub struct SetupService<'a> {
    ctx: &'a AppContext,
    /// `None` reads the system at each call (on WSL that spawns `reg.exe`).
    env: Option<DetectEnv>,
}

fn candidate_dto(
    root: &Path,
    source: wolluf_source_osu::install::CandidateSource,
    result: Result<InstallInfo, SourceError>,
) -> InstallCandidateDto {
    let (valid, osu_db_version, missing) = match result {
        Ok(info) => {
            let missing = (!info.has_scores_db)
                .then(|| MISSING_SCORES_DB.to_owned())
                .into_iter()
                .collect();
            (true, Some(info.osu_db_version), missing)
        }
        Err(SourceError::InvalidInstall { missing, .. }) => (
            false,
            None,
            missing.iter().map(|m| (*m).to_owned()).collect(),
        ),
        Err(_) => (false, None, Vec::new()),
    };
    InstallCandidateDto {
        path: root.to_string_lossy().into_owned(),
        source: source.into(),
        valid,
        osu_db_version,
        missing,
    }
}

fn install_dto(install: &GameInstall) -> Result<InstallDto, AppError> {
    Ok(InstallDto {
        id: u32::try_from(install.id.0)
            .map_err(|_| AppError::internal(format!("install id {} exceeds u32", install.id.0)))?,
        root_path: install.root_path.to_string_lossy().into_owned(),
        osu_db_version: install.client_version,
        detected_at: format_rfc3339_ms(install.detected_at),
    })
}

impl<'a> SetupService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx, env: None }
    }

    /// A fixed environment: tests, and the CLI when it already probed the system.
    pub fn with_env(ctx: &'a AppContext, env: DetectEnv) -> Self {
        Self {
            ctx,
            env: Some(env),
        }
    }

    async fn env(&self) -> Result<DetectEnv, AppError> {
        match &self.env {
            Some(env) => Ok(env.clone()),
            None => tokio::task::spawn_blocking(DetectEnv::from_system)
                .await
                .map_err(blocking_join_error),
        }
    }

    /// Every candidate, invalid explicit ones included, so the UI can say why they failed.
    pub async fn detect_installs(&self) -> Result<Vec<InstallCandidateDto>, AppError> {
        let env = self.env().await?;
        tokio::task::spawn_blocking(move || {
            detect(&env)
                .into_iter()
                .map(|(c, result)| candidate_dto(&c.root, c.source, result))
                .collect()
        })
        .await
        .map_err(blocking_join_error)
    }

    /// Validates, registers through 003 (which runs the data-dir guard) and emits
    /// `DataChanged{setup}`. Invalid → `OSU_DIR_NOT_FOUND` with `args.path`; lazer →
    /// `UNSUPPORTED_FORMAT` (`setup.error.lazer_not_supported`).
    pub async fn set_install_path(&self, path: String) -> Result<InstallDto, AppError> {
        let env = self.env().await?;
        let info = tokio::task::spawn_blocking(move || validate_install(Path::new(&path), &env))
            .await
            .map_err(blocking_join_error)??;
        // DTOs carry paths as strings, so a root that is not UTF-8 could never round-trip.
        if info.root.to_str().is_none() {
            return Err(AppError::invalid_input()
                .with_arg("path", info.root.to_string_lossy().into_owned()));
        }
        let id = self
            .ctx
            .register_install(info.root.clone(), Some(info.osu_db_version))
            .await?;
        let install = self
            .ctx
            .installs()
            .await?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| AppError::internal("install vanished after registration"))?;
        // The install is registered either way; a watcher that cannot start only costs live
        // session updates until the next start.
        if let Err(e) = self.ctx.session().follow_selected_install().await {
            tracing::warn!(
                code = e.code.as_str(),
                details = e.details.as_deref(),
                "install watcher not started"
            );
        }
        self.ctx.emit(AppEvent::data_changed(&[DOMAIN_SETUP]));
        install_dto(&install)
    }

    /// `install` is the most recently registered one: F0 syncs a single install.
    pub async fn status(&self) -> Result<SetupStatusDto, AppError> {
        let install = match self.ctx.installs().await?.into_iter().max_by_key(|i| i.id) {
            Some(i) => Some(install_dto(&i)?),
            None => None,
        };
        let identity_ready = !self.ctx.players().wizard_needed().await?;
        let last_sync = self
            .ctx
            .job_service()
            .list(None)
            .await?
            .into_iter()
            .find(|j| j.kind == JobKindDto::SyncPlays);
        let paths = self.ctx.paths();
        Ok(SetupStatusDto {
            install,
            identity_ready,
            data_dir: paths.data_dir().to_string_lossy().into_owned(),
            logs_dir: paths.logs_dir().to_string_lossy().into_owned(),
            app_version: APP_VERSION.to_owned(),
            last_sync,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use wolluf_core::{ErrorCode, FixedClock};
    use wolluf_source_osu::testkit::FakeInstall;

    use super::*;
    use crate::context::AppPaths;
    use crate::features::players::selection::Decision;
    use crate::features::players::testkit::{PILOT_CFG_USERNAME, pilot_install};
    use crate::features::plays::testkit::{T0, write_file};
    use crate::jobs::JobStatusDto;

    struct Setup {
        dir: tempfile::TempDir,
        ctx: AppContext,
    }

    impl Setup {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let ctx = AppContext::open(
                AppPaths::from_data_dir(dir.path().join("data")),
                Arc::new(FixedClock::new(T0)),
            )
            .unwrap();
            Self { dir, ctx }
        }

        fn install(&self, name: &str, fake: &FakeInstall) -> PathBuf {
            let root = self.dir.path().join(name);
            std::fs::create_dir_all(&root).unwrap();
            for (rel, bytes) in fake.files() {
                write_file(&root, &rel, &bytes);
            }
            root
        }

        fn service(&self) -> SetupService<'_> {
            SetupService::with_env(&self.ctx, DetectEnv::default())
        }
    }

    fn text(p: &Path) -> String {
        p.to_str().unwrap().to_owned()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_valid_path_persists_and_emits_setup_changed() {
        let s = Setup::new();
        let root = s.install("osu!", &FakeInstall::new());
        let mut rx = s.ctx.subscribe();
        let dto = s.service().set_install_path(text(&root)).await.unwrap();
        assert_eq!(dto.root_path, text(&root));
        assert!(dto.osu_db_version.is_some());
        let installs = s.ctx.installs().await.unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(u32::try_from(installs[0].id.0).unwrap(), dto.id);
        assert_eq!(installs[0].client_version, dto.osu_db_version);
        assert_eq!(rx.try_recv().unwrap(), AppEvent::data_changed(&["setup"]));
        let again = s.service().set_install_path(text(&root)).await.unwrap();
        assert_eq!(again, dto, "setting the same path again is an upsert");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_invalid_path_returns_osu_dir_not_found_with_path_arg() {
        let s = Setup::new();
        let empty = s.dir.path().join("not-osu");
        std::fs::create_dir_all(&empty).unwrap();
        for path in [empty, s.dir.path().join("does-not-exist")] {
            let err = s.service().set_install_path(text(&path)).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::OsuDirNotFound, "{path:?}");
            assert_eq!(err.args["path"], text(&path));
        }
        assert!(s.ctx.installs().await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_lazer_returns_unsupported_format() {
        let s = Setup::new();
        let root = s.install("lazer", &FakeInstall::lazer());
        let err = s.service().set_install_path(text(&root)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedFormat);
        assert_eq!(err.message_key, "setup.error.lazer_not_supported");
        assert!(s.ctx.installs().await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn osu_root_containing_data_dir_is_refused() {
        let s = Setup::new();
        // The tempdir holds `data/`, so installing osu! there puts the data dir inside it.
        let root = s.install(".", &FakeInstall::new());
        let err = s.service().set_install_path(text(&root)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert_eq!(err.message_key, "error.data_dir_inside_osu");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_without_install() {
        let s = Setup::new();
        let status = s.service().status().await.unwrap();
        assert_eq!(status.install, None);
        assert!(status.identity_ready, "nothing to confirm without plays");
        assert_eq!(status.last_sync, None);
        assert_eq!(status.data_dir, text(&s.dir.path().join("data")));
        assert_eq!(
            status.logs_dir,
            text(&s.dir.path().join("data").join("logs"))
        );
        assert_eq!(status.app_version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_identity_ready_follows_players_service() {
        let s = Setup::new();
        let root = s.install("osu!", &pilot_install(Some(PILOT_CFG_USERNAME)));
        let install = s.service().set_install_path(text(&root)).await.unwrap();
        let job = s
            .ctx
            .plays()
            .sync_and_wait(wolluf_store::repo::ledger::InstallId(i64::from(install.id)))
            .await
            .unwrap();
        let status = s.service().status().await.unwrap();
        assert_eq!(status.install.as_ref(), Some(&install));
        assert!(!status.identity_ready);
        assert_eq!(
            status.identity_ready,
            !s.ctx.players().wizard_needed().await.unwrap()
        );
        let last = status.last_sync.unwrap();
        assert_eq!(last.id, job.id);
        assert_eq!(last.status, JobStatusDto::Ok);

        let first = s.ctx.players().list_aliases().await.unwrap().rows[0].alias_id;
        s.ctx
            .players()
            .decide(vec![(first, Some(Decision::Me))], true)
            .await
            .unwrap();
        assert!(s.service().status().await.unwrap().identity_ready);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detect_reports_env_candidates_valid_and_invalid() {
        let s = Setup::new();
        let root = s.install("osu!", &FakeInstall::new().without("scores.db"));
        let env = DetectEnv {
            osu_dir_override: Some(text(&root)),
            ..DetectEnv::default()
        };
        let found = SetupService::with_env(&s.ctx, env)
            .detect_installs()
            .await
            .unwrap();
        let env_candidate = found
            .iter()
            .find(|c| c.source == dto::InstallSourceDto::Env)
            .unwrap();
        assert!(env_candidate.valid);
        assert_eq!(env_candidate.missing, vec!["scores.db".to_owned()]);
        assert!(env_candidate.osu_db_version.is_some());

        let empty = s.dir.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let env = DetectEnv {
            osu_dir_override: Some(text(&empty)),
            ..DetectEnv::default()
        };
        let found = SetupService::with_env(&s.ctx, env)
            .detect_installs()
            .await
            .unwrap();
        let invalid = found
            .iter()
            .find(|c| c.source == dto::InstallSourceDto::Env)
            .unwrap();
        assert!(!invalid.valid);
        assert_eq!(
            invalid.missing,
            vec!["osu!.db".to_owned(), "osu!.exe".to_owned()]
        );
    }
}

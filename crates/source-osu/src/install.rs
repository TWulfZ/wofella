//! osu! stable install discovery and validation (spec 002 "Install detection"). The single
//! owner of detection: 005's setup service and the CLI only call it.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cfg_files::{CfgFile, list_user_cfgs, read_user_cfg};
use crate::codec::FileKind;
use crate::error::{CodecError, SourceError};
use crate::paths::{
    WSL_MOUNT_ROOT, is_drvfs_path, is_windows_absolute, join_under, resolve_songs_dir,
    windows_join, windows_to_wsl_under,
};
use crate::process::{DEFAULT_TIMEOUT, run_with_timeout};
use crate::stable::stable_str_enum;

const OSU_DB: &str = "osu!.db";
const OSU_EXE: &str = "osu!.exe";
const SCORES_DB: &str = "scores.db";
const COLLECTION_DB: &str = "collection.db";
/// lazer's database; a folder with it and no osu!.db is a lazer install.
const LAZER_MARKER: &str = "client.realm";
const DATA_DIR: &str = "Data";
const REPLAY_DIR: &str = "r";
const OSU_DIR_NAME: &str = "osu!";
const GAMES_DIR_NAME: &str = "Games";
const APPDATA_LOCAL: [&str; 2] = ["AppData", "Local"];
const MISSING_DIRECTORY: &str = "directory";
const ENV_OSU_DIR: &str = "WOLLUF_OSU_DIR";
/// Verified on the pilot 2026-09-28 (`"E:\Games\osu!\osu!.exe" "%1"`); `HKCR\osu` is absent.
const REGISTRY_KEY: &str = r"osustable.File.osz\shell\open\command";
const WSL_REG_EXE: &str = "/mnt/c/Windows/System32/reg.exe";
const WSL_INTEROP_MARKER: &str = "/proc/sys/fs/binfmt_misc/WSLInterop";
const WSL_USERS_DIR: &str = "/mnt/c/Users";
const WSL_SKIPPED_PROFILES: [&str; 4] = ["Public", "Default", "Default User", "All Users"];
const OSU_DB_VERSION_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    Windows,
    Wsl,
    #[default]
    Linux,
    Other,
}

stable_str_enum! {
    /// Where a candidate root came from; stable strings shown by 005's setup screen.
    pub enum CandidateSource {
        Env => "env",
        Registry => "registry",
        LocalAppData => "local_app_data",
        WslUserProfile => "wsl_user_profile",
        DriveScan => "drive_scan",
        ProgramFiles => "program_files",
    }
}

impl CandidateSource {
    /// Guessed locations are dropped by [`detect`] when they do not exist; explicit ones are
    /// always reported so the UI can say why they failed.
    const fn is_explicit(self) -> bool {
        matches!(self, Self::Env | Self::Registry)
    }
}

/// Everything detection reads from the system, gathered once so [`candidate_roots`] is pure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectEnv {
    pub platform: Platform,
    pub osu_dir_override: Option<String>,
    pub local_app_data: Option<String>,
    pub program_files: Vec<String>,
    pub drive_roots: Vec<PathBuf>,
    pub wsl_user_profiles: Vec<PathBuf>,
    pub registry_open_command: Option<String>,
    pub wsl_mount_root: PathBuf,
}

impl Default for DetectEnv {
    fn default() -> Self {
        Self {
            platform: Platform::default(),
            osu_dir_override: None,
            local_app_data: None,
            program_files: Vec::new(),
            drive_roots: Vec::new(),
            wsl_user_profiles: Vec::new(),
            registry_open_command: None,
            wsl_mount_root: PathBuf::from(WSL_MOUNT_ROOT),
        }
    }
}

impl DetectEnv {
    pub fn from_system() -> Self {
        Self::from_system_with_timeout(DEFAULT_TIMEOUT)
    }

    /// A source that fails is logged at debug and skipped.
    pub fn from_system_with_timeout(timeout: Duration) -> Self {
        let platform = current_platform();
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let program_files = ["ProgramFiles", "ProgramFiles(x86)"]
            .iter()
            .filter_map(|n| var(n))
            .collect();
        let (drive_roots, wsl_user_profiles) = match platform {
            Platform::Windows => (windows_drive_roots(), Vec::new()),
            Platform::Wsl => (wsl_drive_roots(), wsl_user_profiles()),
            Platform::Linux | Platform::Other => (Vec::new(), Vec::new()),
        };
        Self {
            platform,
            osu_dir_override: var(ENV_OSU_DIR),
            local_app_data: var("LOCALAPPDATA"),
            program_files,
            drive_roots,
            wsl_user_profiles,
            registry_open_command: read_registry(platform, timeout),
            wsl_mount_root: PathBuf::from(WSL_MOUNT_ROOT),
        }
    }

    /// A user-typed Windows path on WSL is translated; everything else is taken as is.
    fn host_path(&self, raw: &str) -> PathBuf {
        if self.platform == Platform::Wsl
            && is_windows_absolute(raw)
            && let Some(p) = windows_to_wsl_under(raw, &self.wsl_mount_root)
        {
            return p;
        }
        PathBuf::from(raw)
    }

    fn join(&self, base: &Path, rel: &[&str]) -> PathBuf {
        join_under(base, rel, self.platform)
    }
}

pub(crate) fn current_platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else if Path::new(WSL_INTEROP_MARKER).exists()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
    {
        Platform::Wsl
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else {
        Platform::Other
    }
}

fn windows_drive_roots() -> Vec<PathBuf> {
    (b'C'..=b'Z')
        .map(|d| PathBuf::from(format!("{}:\\", char::from(d))))
        .filter(|p| p.is_dir())
        .collect()
}

fn sorted_dirs(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        tracing::debug!(dir = %dir.display(), "detect: cannot list directory");
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_str().is_some_and(&keep))
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

fn wsl_drive_roots() -> Vec<PathBuf> {
    sorted_dirs(Path::new(WSL_MOUNT_ROOT), |n| {
        n.len() == 1 && n.as_bytes()[0].is_ascii_lowercase()
    })
}

fn wsl_user_profiles() -> Vec<PathBuf> {
    sorted_dirs(Path::new(WSL_USERS_DIR), |n| {
        !WSL_SKIPPED_PROFILES.contains(&n)
    })
}

fn read_registry(platform: Platform, timeout: Duration) -> Option<String> {
    match platform {
        Platform::Windows => read_registry_native(),
        Platform::Wsl => read_registry_via_interop(timeout),
        Platform::Linux | Platform::Other => None,
    }
}

#[cfg(windows)]
fn read_registry_native() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CLASSES_ROOT;
    let value = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(REGISTRY_KEY)
        .and_then(|key| key.get_value::<String, _>(""));
    match value {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::debug!(error = %e, "detect: registry source unavailable");
            None
        }
    }
}

#[cfg(not(windows))]
fn read_registry_native() -> Option<String> {
    None
}

fn read_registry_via_interop(timeout: Duration) -> Option<String> {
    let mut cmd = std::process::Command::new(WSL_REG_EXE);
    cmd.args(["query", &format!(r"HKCR\{REGISTRY_KEY}"), "/ve"]);
    match run_with_timeout(cmd, timeout) {
        Ok(out) => parse_reg_query(&String::from_utf8_lossy(&out)),
        Err(e) => {
            tracing::debug!(error = %e, "detect: reg.exe source unavailable");
            None
        }
    }
}

/// Value of the default entry in `reg.exe query … /ve` output.
pub fn parse_reg_query(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, rest) = line
            .split_once("REG_SZ")
            .or_else(|| line.split_once("REG_EXPAND_SZ"))?;
        let value = rest.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// The executable of a shell open command: `"E:\Games\osu!\osu!.exe" "%1"` → the quoted path.
pub fn parse_open_command(command: &str) -> Option<String> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        let exe = &rest[..end];
        return (!exe.is_empty()).then(|| exe.to_owned());
    }
    let lower = command.to_ascii_lowercase();
    let end = match lower.find(".exe") {
        Some(i) => i + ".exe".len(),
        None => command.find(char::is_whitespace).unwrap_or(command.len()),
    };
    let exe = command[..end].trim();
    (!exe.is_empty()).then(|| exe.to_owned())
}

fn parent_of_windows_path(path: &str) -> Option<&str> {
    let cut = path.rfind(['\\', '/'])?;
    let parent = &path[..cut];
    (!parent.is_empty()).then_some(parent)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub root: PathBuf,
    pub source: CandidateSource,
}

/// Windows and drvfs compare names case-insensitively; native Linux does not.
pub(crate) fn dedup_key(platform: Platform, path: &Path) -> String {
    let text = path.to_string_lossy();
    if platform == Platform::Windows {
        text.replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    } else if is_drvfs_path(path) {
        text.trim_end_matches('/').to_lowercase()
    } else {
        text.trim_end_matches('/').to_string()
    }
}

/// Ordered, deduplicated candidates; the first source wins on a duplicate.
pub fn candidate_roots(env: &DetectEnv) -> Vec<Candidate> {
    let mut raw: Vec<Candidate> = Vec::new();
    let mut push = |root: PathBuf, source| raw.push(Candidate { root, source });
    if let Some(dir) = &env.osu_dir_override {
        push(env.host_path(dir), CandidateSource::Env);
    }
    if let Some(parent) = env
        .registry_open_command
        .as_deref()
        .and_then(parse_open_command)
        .as_deref()
        .and_then(parent_of_windows_path)
    {
        push(env.host_path(parent), CandidateSource::Registry);
    }
    if let Some(local) = &env.local_app_data {
        push(
            env.join(&env.host_path(local), &[OSU_DIR_NAME]),
            CandidateSource::LocalAppData,
        );
    }
    for profile in &env.wsl_user_profiles {
        let mut rel = APPDATA_LOCAL.to_vec();
        rel.push(OSU_DIR_NAME);
        push(env.join(profile, &rel), CandidateSource::WslUserProfile);
    }
    for drive in &env.drive_roots {
        push(env.join(drive, &[OSU_DIR_NAME]), CandidateSource::DriveScan);
        push(
            env.join(drive, &[GAMES_DIR_NAME, OSU_DIR_NAME]),
            CandidateSource::DriveScan,
        );
    }
    for pf in &env.program_files {
        let base = if env.platform == Platform::Windows {
            windows_join(pf, &[OSU_DIR_NAME])
        } else {
            env.join(&env.host_path(pf), &[OSU_DIR_NAME])
        };
        push(base, CandidateSource::ProgramFiles);
    }
    let mut seen = BTreeSet::new();
    raw.into_iter()
        .filter(|c| seen.insert(dedup_key(env.platform, &c.root)))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallInfo {
    pub root: PathBuf,
    /// Header of osu!.db: the client build that last wrote it (ADR 0015).
    pub osu_db_version: i32,
    pub has_scores_db: bool,
    pub has_collection_db: bool,
    pub has_data_r: bool,
    /// Newest first.
    pub user_cfgs: Vec<CfgFile>,
    pub songs_dir: PathBuf,
}

/// Directory entries by lowercase name, because osu! runs on case-insensitive file systems.
fn entries_by_lowercase_name(dir: &Path) -> Result<Vec<(String, PathBuf)>, SourceError> {
    let entries = fs::read_dir(dir).map_err(|e| SourceError::io(dir, &e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| SourceError::io(dir, &e))?;
        if let Some(name) = entry.file_name().to_str() {
            out.push((name.to_lowercase(), entry.path()));
        }
    }
    out.sort();
    Ok(out)
}

fn find(entries: &[(String, PathBuf)], name: &str) -> Option<PathBuf> {
    let lower = name.to_lowercase();
    entries
        .iter()
        .find(|(n, _)| *n == lower)
        .map(|(_, p)| p.clone())
}

fn read_osu_db_version(path: &Path) -> Result<i32, SourceError> {
    let mut file = File::open(path).map_err(|e| SourceError::io(path, &e))?;
    let mut buf = [0u8; OSU_DB_VERSION_LEN];
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => {
                return Err(CodecError::Truncated {
                    kind: FileKind::OsuDb,
                    offset: filled as u64,
                    needed: (OSU_DB_VERSION_LEN - filled) as u64,
                }
                .into());
            }
            Ok(n) => filled += n,
            Err(e) => return Err(SourceError::io(path, &e)),
        }
    }
    Ok(i32::from_le_bytes(buf))
}

/// A file path is normalised to its directory; a Windows path on WSL is translated first.
pub fn validate_install(path: &Path, env: &DetectEnv) -> Result<InstallInfo, SourceError> {
    let mut root = match path.to_str() {
        Some(raw) => env.host_path(raw),
        None => path.to_path_buf(),
    };
    if root.is_file()
        && let Some(parent) = root.parent()
    {
        root = parent.to_path_buf();
    }
    if !root.is_dir() {
        return Err(SourceError::InvalidInstall {
            path: root,
            missing: vec![MISSING_DIRECTORY],
        });
    }
    let entries = entries_by_lowercase_name(&root)?;
    let osu_db = find(&entries, OSU_DB).filter(|p| p.is_file());
    let osu_exe = find(&entries, OSU_EXE).filter(|p| p.is_file());
    if osu_db.is_none() && find(&entries, LAZER_MARKER).is_some() {
        return Err(SourceError::LazerInstall { path: root });
    }
    let (osu_db, _) = match (osu_db, osu_exe) {
        (Some(db), Some(exe)) => (db, exe),
        (db, exe) => {
            let missing = [(OSU_DB, db.is_some()), (OSU_EXE, exe.is_some())]
                .into_iter()
                .filter_map(|(name, present)| (!present).then_some(name))
                .collect();
            return Err(SourceError::InvalidInstall {
                path: root,
                missing,
            });
        }
    };
    let osu_db_version = read_osu_db_version(&osu_db)?;
    let has_data_r = find(&entries, DATA_DIR)
        .filter(|p| p.is_dir())
        .and_then(|data| entries_by_lowercase_name(&data).ok())
        .and_then(|data| find(&data, REPLAY_DIR))
        .is_some_and(|r| r.is_dir());
    let user_cfgs = list_user_cfgs(&root)?;
    let beatmap_directory = user_cfgs
        .first()
        .and_then(|cfg| match read_user_cfg(&cfg.path) {
            Ok((cfg, _)) => cfg.beatmap_directory,
            Err(e) => {
                tracing::debug!(error = %e, "detect: newest cfg unreadable, using default Songs");
                None
            }
        });
    let songs_dir = resolve_songs_dir(&root, beatmap_directory.as_deref(), env.platform);
    Ok(InstallInfo {
        has_scores_db: find(&entries, SCORES_DB).is_some_and(|p| p.is_file()),
        has_collection_db: find(&entries, COLLECTION_DB).is_some_and(|p| p.is_file()),
        has_data_r,
        osu_db_version,
        user_cfgs,
        songs_dir,
        root,
    })
}

/// Validates candidates in order and keeps invalid explicit ones (env, registry) so the UI can
/// say why; guessed locations that do not exist are dropped. No candidate → empty vec.
pub fn detect(env: &DetectEnv) -> Vec<(Candidate, Result<InstallInfo, SourceError>)> {
    candidate_roots(env)
        .into_iter()
        .filter(|c| c.source.is_explicit() || c.root.exists())
        .map(|c| {
            let result = validate_install(&c.root, env);
            (c, result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(platform: Platform) -> DetectEnv {
        DetectEnv {
            platform,
            ..DetectEnv::default()
        }
    }

    fn roots(env: &DetectEnv) -> Vec<(String, CandidateSource)> {
        candidate_roots(env)
            .into_iter()
            .map(|c| (c.root.to_string_lossy().into_owned(), c.source))
            .collect()
    }

    #[test]
    fn env_candidate_first() {
        let env = DetectEnv {
            osu_dir_override: Some("/data/osu".into()),
            registry_open_command: Some(r#""E:\Games\osu!\osu!.exe" "%1""#.into()),
            drive_roots: vec![PathBuf::from("/mnt/e")],
            ..empty(Platform::Wsl)
        };
        let got = roots(&env);
        assert_eq!(got[0], ("/data/osu".to_owned(), CandidateSource::Env));
        assert_eq!(
            got[1],
            ("/mnt/e/Games/osu!".to_owned(), CandidateSource::Registry)
        );
        let translated = DetectEnv {
            osu_dir_override: Some(r"E:\osu!".into()),
            ..empty(Platform::Wsl)
        };
        assert_eq!(roots(&translated)[0].0, "/mnt/e/osu!");
    }

    #[test]
    fn parses_osz_open_command() {
        assert_eq!(
            parse_open_command(r#""E:\Games\osu!\osu!.exe" "%1""#).as_deref(),
            Some(r"E:\Games\osu!\osu!.exe")
        );
        assert_eq!(
            parse_open_command(r"C:\Program Files\osu!\osu!.EXE %1").as_deref(),
            Some(r"C:\Program Files\osu!\osu!.EXE")
        );
        assert_eq!(
            parse_open_command(r#"  "D:\osu!\osu!.exe""#).as_deref(),
            Some(r"D:\osu!\osu!.exe")
        );
        assert_eq!(parse_open_command(""), None);
        assert_eq!(parse_open_command(r#""unterminated"#), None);
        assert_eq!(
            parse_reg_query(
                "\r\nHKEY_CLASSES_ROOT\\osustable.File.osz\\shell\\open\\command\r\n    (Default)    REG_SZ    \"E:\\Games\\osu!\\osu!.exe\" \"%1\"\r\n\r\n"
            )
            .as_deref(),
            Some(r#""E:\Games\osu!\osu!.exe" "%1""#)
        );
        assert_eq!(
            parse_reg_query("ERROR: The system was unable to find the key"),
            None
        );
    }

    #[test]
    fn registry_candidate_translated_on_wsl() {
        let cmd = Some(r#""E:\Games\osu!\osu!.exe" "%1""#.to_owned());
        let wsl = DetectEnv {
            registry_open_command: cmd.clone(),
            ..empty(Platform::Wsl)
        };
        assert_eq!(
            roots(&wsl),
            vec![("/mnt/e/Games/osu!".to_owned(), CandidateSource::Registry)]
        );
        let win = DetectEnv {
            registry_open_command: cmd,
            ..empty(Platform::Windows)
        };
        assert_eq!(
            roots(&win),
            vec![(r"E:\Games\osu!".to_owned(), CandidateSource::Registry)]
        );
    }

    #[test]
    fn candidates_windows_order() {
        let env = DetectEnv {
            osu_dir_override: Some(r"F:\custom".into()),
            registry_open_command: Some(r#""E:\Games\osu!\osu!.exe" "%1""#.into()),
            local_app_data: Some(r"C:\Users\u\AppData\Local".into()),
            drive_roots: vec![PathBuf::from(r"C:\"), PathBuf::from(r"D:\")],
            program_files: vec![r"C:\Program Files".into(), r"C:\Program Files (x86)".into()],
            ..empty(Platform::Windows)
        };
        assert_eq!(
            roots(&env),
            vec![
                (r"F:\custom".to_owned(), CandidateSource::Env),
                (r"E:\Games\osu!".to_owned(), CandidateSource::Registry),
                (
                    r"C:\Users\u\AppData\Local\osu!".to_owned(),
                    CandidateSource::LocalAppData
                ),
                (r"C:\osu!".to_owned(), CandidateSource::DriveScan),
                (r"C:\Games\osu!".to_owned(), CandidateSource::DriveScan),
                (r"D:\osu!".to_owned(), CandidateSource::DriveScan),
                (r"D:\Games\osu!".to_owned(), CandidateSource::DriveScan),
                (
                    r"C:\Program Files\osu!".to_owned(),
                    CandidateSource::ProgramFiles
                ),
                (
                    r"C:\Program Files (x86)\osu!".to_owned(),
                    CandidateSource::ProgramFiles
                ),
            ]
        );
    }

    #[test]
    fn candidates_wsl_include_mnt_games() {
        let env = DetectEnv {
            drive_roots: vec![PathBuf::from("/mnt/c"), PathBuf::from("/mnt/e")],
            wsl_user_profiles: vec![PathBuf::from("/mnt/c/Users/u")],
            ..empty(Platform::Wsl)
        };
        assert_eq!(
            roots(&env),
            vec![
                (
                    "/mnt/c/Users/u/AppData/Local/osu!".to_owned(),
                    CandidateSource::WslUserProfile
                ),
                ("/mnt/c/osu!".to_owned(), CandidateSource::DriveScan),
                ("/mnt/c/Games/osu!".to_owned(), CandidateSource::DriveScan),
                ("/mnt/e/osu!".to_owned(), CandidateSource::DriveScan),
                ("/mnt/e/Games/osu!".to_owned(), CandidateSource::DriveScan),
            ]
        );
    }

    #[test]
    fn candidates_deduplicated_case_insensitive() {
        let win = DetectEnv {
            registry_open_command: Some(r#""e:\games\OSU!\osu!.exe" "%1""#.into()),
            drive_roots: vec![PathBuf::from(r"E:\")],
            ..empty(Platform::Windows)
        };
        assert_eq!(
            roots(&win),
            vec![
                (r"e:\games\OSU!".to_owned(), CandidateSource::Registry),
                (r"E:\osu!".to_owned(), CandidateSource::DriveScan),
            ]
        );
        let wsl = DetectEnv {
            registry_open_command: Some(r#""E:\games\OSU!\osu!.exe" "%1""#.into()),
            drive_roots: vec![PathBuf::from("/mnt/e")],
            ..empty(Platform::Wsl)
        };
        let got = roots(&wsl);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].1, CandidateSource::Registry);
        // Native Linux paths are case-sensitive: no folding there.
        let linux = DetectEnv {
            osu_dir_override: Some("/data/OSU".into()),
            drive_roots: vec![],
            ..empty(Platform::Linux)
        };
        assert_eq!(roots(&linux).len(), 1);
        assert!(
            dedup_key(Platform::Linux, Path::new("/a/B"))
                != dedup_key(Platform::Linux, Path::new("/a/b"))
        );
    }

    #[test]
    fn candidates_tagged_with_source() {
        let strings: Vec<&str> = CandidateSource::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            strings,
            [
                "env",
                "registry",
                "local_app_data",
                "wsl_user_profile",
                "drive_scan",
                "program_files"
            ]
        );
        let env = DetectEnv {
            registry_open_command: Some(r#""E:\Games\osu!\osu!.exe" "%1""#.into()),
            drive_roots: vec![PathBuf::from("/mnt/e")],
            ..empty(Platform::Wsl)
        };
        // The pilot is found by `registry` and again by `drive_scan`; dedup keeps the first.
        let got = roots(&env);
        assert_eq!(
            got.iter().filter(|(r, _)| r == "/mnt/e/Games/osu!").count(),
            1
        );
        assert_eq!(
            got.iter()
                .find(|(r, _)| r == "/mnt/e/Games/osu!")
                .unwrap()
                .1,
            CandidateSource::Registry
        );
    }
}

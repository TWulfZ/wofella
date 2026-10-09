//! The only module that writes into the osu! folder (architecture D9, ADR 0025). Every write
//! takes an [`ExportPermit`], and the only way to get one is [`confirm`] on a preview the user
//! was shown. Writes create new files in one set folder and never replace an existing one.
//!
//! No collection.db is written, so D9's osu!-not-running check and backup do not apply yet.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use wolluf_core::UnixUs;

use crate::errors::AppError;
use crate::jobs::to_system_time;

pub mod keys {
    /// The set folder is not a folder under the install's Songs dir.
    pub const FOLDER_OUTSIDE_SONGS: &str = "export.error.folder_outside_songs";
    /// The preview was never recorded, expired, or was already confirmed.
    pub const PREVIEW_UNKNOWN: &str = "export.error.preview_unknown";
    /// Something other than a regular file (a folder, a link) holds the target name.
    pub const TARGET_NOT_A_FILE: &str = "export.error.target_not_a_file";
}

/// stable opens files through Win32 paths without the `\\?\` prefix: `MAX_PATH` (260) less the
/// terminating NUL, in UTF-16 units.
pub(crate) const MAX_PATH_UNITS: usize = 259;
/// NTFS caps a name at 255 UTF-16 units and ext4 at 255 bytes; bytes bound both.
pub(crate) const MAX_NAME_BYTES: usize = 255;
/// Unpublished content is written under this prefix; stable lists neither `.tmp` nor dot files.
pub(crate) const TEMP_PREFIX: &str = ".wolluf-";

const US_PER_MINUTE: i64 = 60_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportParams {
    /// A preview older than this is gone: the files on disk may have changed since it was shown.
    pub preview_ttl_us: i64,
    /// Previews hold whole rewritten charts in memory; the oldest go first.
    pub max_previews: usize,
}

impl Default for ExportParams {
    fn default() -> Self {
        Self {
            preview_ttl_us: 15 * US_PER_MINUTE,
            max_previews: 32,
        }
    }
}

/// A set folder under the install's Songs dir, resolved through symlinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetFolder {
    songs: PathBuf,
    path: PathBuf,
    /// As the user knows it: canonical paths on Windows carry a `\\?\` prefix.
    shown: PathBuf,
}

impl SetFolder {
    /// The folder holding `chart_rel_path` (a catalog path, relative to `songs_dir`).
    pub(crate) fn resolve(songs_dir: &Path, chart_rel_path: &Path) -> Result<Self, AppError> {
        let outside = || {
            AppError::invalid_input()
                .with_key(keys::FOLDER_OUTSIDE_SONGS)
                .with_arg("path", chart_rel_path.to_string_lossy())
        };
        let rel = chart_rel_path
            .parent()
            .filter(|p| {
                p.components().next().is_some()
                    && p.components().all(|c| matches!(c, Component::Normal(_)))
            })
            .ok_or_else(outside)?;
        let missing = |e: io::Error| match e.kind() {
            io::ErrorKind::NotFound => {
                AppError::not_found().with_arg("path", chart_rel_path.to_string_lossy())
            }
            _ => AppError::internal(format!("resolve {}: {e}", rel.display())),
        };
        let songs = std::fs::canonicalize(songs_dir).map_err(missing)?;
        let path = std::fs::canonicalize(songs_dir.join(rel)).map_err(missing)?;
        // Canonical on both sides, so a symlinked set folder cannot lead the writes elsewhere.
        if path == songs || !path.starts_with(&songs) {
            return Err(outside());
        }
        if !path.is_dir() {
            return Err(missing(io::ErrorKind::NotFound.into()));
        }
        Ok(Self {
            songs,
            path,
            shown: songs_dir.join(rel),
        })
    }

    /// The folder may have been swapped for a link since it was resolved.
    fn still_inside(&self) -> Result<(), AppError> {
        let now = std::fs::canonicalize(&self.path)
            .map_err(|e| AppError::internal(format!("resolve {}: {e}", self.path.display())))?;
        if now == self.path && now.starts_with(&self.songs) {
            Ok(())
        } else {
            Err(AppError::invalid_input()
                .with_key(keys::FOLDER_OUTSIDE_SONGS)
                .with_arg("path", self.path.to_string_lossy()))
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn shown(&self) -> &Path {
        &self.shown
    }

    /// `name` as one entry of this folder; anything else is refused before any IO. Names stable
    /// could not open on Windows are refused here too, so the plan reports them before a write.
    pub(crate) fn entry(&self, name: &str) -> Result<PathBuf, AppError> {
        let single = !name.trim().is_empty()
            && !name.contains(['/', '\\', '\0', ':', '<', '>', '"', '|', '?', '*'])
            && name.len() <= MAX_NAME_BYTES
            && matches!(
                Path::new(name).components().collect::<Vec<_>>()[..],
                [Component::Normal(_)]
            );
        let fits = || {
            self.shown
                .join(name)
                .to_string_lossy()
                .encode_utf16()
                .count()
                <= MAX_PATH_UNITS
        };
        if single && fits() {
            Ok(self.path.join(name))
        } else {
            Err(AppError::invalid_input().with_arg("fileName", name))
        }
    }

    pub(crate) fn has_file(&self, name: &str) -> Result<bool, AppError> {
        Ok(self.entry(name)?.is_file())
    }
}

/// What the user was shown: the folder, the only file names a permit for it may create, and
/// the feature's own plan.
#[derive(Debug, Clone)]
pub(crate) struct Preview<T> {
    pub(crate) folder: SetFolder,
    pub(crate) files: Vec<String>,
    pub(crate) payload: T,
}

/// In memory only: a restart forgets every preview, so nothing confirmed can outlive the
/// process that showed it.
#[derive(Debug)]
pub struct PreviewRegistry<T> {
    params: ExportParams,
    state: Mutex<Recorded<T>>,
}

#[derive(Debug)]
struct Recorded<T> {
    /// Monotonic, so the map's first key is the oldest preview.
    ids: ulid::Generator,
    entries: BTreeMap<String, (UnixUs, Preview<T>)>,
}

impl<T> PreviewRegistry<T> {
    pub fn new(params: ExportParams) -> Self {
        Self {
            params,
            state: Mutex::new(Recorded {
                ids: ulid::Generator::new(),
                entries: BTreeMap::new(),
            }),
        }
    }

    /// Returns the preview id `confirm` takes.
    pub(crate) fn record(&self, now: UnixUs, preview: Preview<T>) -> String {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let id = match state.ids.generate_from_datetime(to_system_time(now)) {
            Ok(ulid) => ulid,
            Err(overflow) => overflow.commit_overflow_increment(),
        }
        .to_string();
        let ttl = self.params.preview_ttl_us;
        state.entries.retain(|_, (at, _)| now.0 - at.0 <= ttl);
        while state.entries.len() >= self.params.max_previews.max(1) {
            state.entries.pop_first();
        }
        state.entries.insert(id.clone(), (now, preview));
        id
    }

    fn take(&self, preview_id: &str, now: UnixUs) -> Option<Preview<T>> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (at, preview) = state.entries.remove(preview_id)?;
        (now.0 - at.0 <= self.params.preview_ttl_us).then_some(preview)
    }
}

/// Proof that the user confirmed one recorded preview. The private fields make [`confirm`] the
/// only constructor (D9; `tests/export_permit.rs` checks it does not compile elsewhere).
#[derive(Debug)]
pub struct ExportPermit {
    preview_id: String,
    folder: SetFolder,
    files: Vec<String>,
}

impl ExportPermit {
    pub fn preview_id(&self) -> &str {
        &self.preview_id
    }

    pub fn folder(&self) -> &SetFolder {
        &self.folder
    }
}

/// Consumes the preview: one confirmation mints one permit.
pub(crate) fn confirm<T>(
    registry: &PreviewRegistry<T>,
    preview_id: &str,
    now: UnixUs,
) -> Result<(ExportPermit, T), AppError> {
    let Some(preview) = registry.take(preview_id, now) else {
        return Err(AppError::not_found()
            .with_key(keys::PREVIEW_UNKNOWN)
            .with_arg("previewId", preview_id));
    };
    let permit = ExportPermit {
        preview_id: preview_id.to_owned(),
        folder: preview.folder,
        files: preview.files,
    };
    Ok((permit, preview.payload))
}

#[derive(Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Written(Published),
    /// Left untouched: a file of that name was already there.
    Exists(PathBuf),
}

/// A file [`write_new`] created. Only this module builds one, so [`retract`] can only remove
/// what a permit itself wrote.
#[derive(Debug, PartialEq, Eq)]
pub struct Published {
    path: PathBuf,
}

impl Published {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Creates `name` in the permit's folder with `bytes`. An existing entry is never replaced, and
/// the name never holds partial content: the bytes are synced under a temp name first, then
/// published in one step.
pub fn write_new(
    permit: &ExportPermit,
    name: &str,
    bytes: &[u8],
) -> Result<WriteOutcome, AppError> {
    let path = permit.folder.entry(name)?;
    if !permit.files.iter().any(|f| f == name) {
        return Err(AppError::invalid_input()
            .with_arg("fileName", name)
            .with_details("not a file of the confirmed preview"));
    }
    permit.folder.still_inside()?;
    let tmp = TempFile::create(permit.folder.path(), bytes)?;
    // Rendering and syncing take a while; the folder may have been swapped meanwhile.
    permit.folder.still_inside()?;
    publish(tmp, &path, |from, to| std::fs::hard_link(from, to))
}

/// Removes a file this permit wrote: undoes one half of a copy whose other half failed.
pub fn retract(permit: &ExportPermit, published: Published) -> Result<(), AppError> {
    if published.path.parent() != Some(permit.folder.path()) {
        return Err(AppError::invalid_input()
            .with_arg("path", published.path.to_string_lossy())
            .with_details("not in the permit's folder"));
    }
    permit.folder.still_inside()?;
    std::fs::remove_file(&published.path)
        .map_err(|e| AppError::internal(format!("remove {}: {e}", published.path.display())))
}

/// `link` is `std::fs::hard_link`: it creates `target` from the synced temp file and fails if
/// the name is taken, where `rename` would replace it (POSIX, and `MoveFileEx` as std calls it).
/// A volume without hard links (FAT32/exFAT) falls back to reserving `target` with `create_new`
/// and renaming over that empty placeholder, which only this call created; something swapping
/// the placeholder within that window is the one case a rename can still replace.
fn publish(
    tmp: TempFile,
    target: &Path,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<WriteOutcome, AppError> {
    let failed =
        |what: &str, e: io::Error| AppError::internal(format!("{what} {}: {e}", target.display()));
    match link(&tmp.path, target) {
        Ok(()) => {
            return Ok(WriteOutcome::Written(Published {
                path: target.to_owned(),
            }));
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return existing(target),
        // Any other failure (EPERM/EOPNOTSUPP on Unix, ERROR_INVALID_FUNCTION on FAT) is taken
        // as "no hard links here"; a real IO fault fails the reservation below as well.
        Err(_) => {}
    }
    match OpenOptions::new().write(true).create_new(true).open(target) {
        Ok(placeholder) => drop(placeholder),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return existing(target),
        // Windows answers CREATE_NEW on a folder with ERROR_ACCESS_DENIED, not ALREADY_EXISTS.
        Err(_) if std::fs::symlink_metadata(target).is_ok() => return existing(target),
        Err(e) => return Err(failed("reserve", e)),
    }
    if let Err(e) = std::fs::rename(&tmp.path, target) {
        let _ = std::fs::remove_file(target);
        return Err(failed("publish", e));
    }
    tmp.consumed();
    Ok(WriteOutcome::Written(Published {
        path: target.to_owned(),
    }))
}

/// A taken name is only "already there" when it holds a regular file; a folder or a link at that
/// name would otherwise pass for a copy that was skipped.
fn existing(target: &Path) -> Result<WriteOutcome, AppError> {
    match std::fs::symlink_metadata(target) {
        Ok(meta) if meta.file_type().is_file() => Ok(WriteOutcome::Exists(target.to_owned())),
        Ok(_) => Err(AppError::conflict()
            .with_key(keys::TARGET_NOT_A_FILE)
            .with_arg("path", target.to_string_lossy())),
        Err(e) => Err(AppError::internal(format!(
            "inspect {}: {e}",
            target.display()
        ))),
    }
}

/// Synced content under a unique name of the target's folder (same volume, so publishing never
/// copies). Dropped unpublished, it removes itself.
#[derive(Debug)]
struct TempFile {
    path: PathBuf,
    published: bool,
}

/// Unique within the process; `create_new` settles clashes with other processes.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
const TEMP_ATTEMPTS: u32 = 16;

impl TempFile {
    fn create(dir: &Path, bytes: &[u8]) -> Result<Self, AppError> {
        let mut attempts = 0;
        let (path, mut file) = loop {
            let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = dir.join(format!("{TEMP_PREFIX}{}-{seq}.tmp", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => break (path, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < TEMP_ATTEMPTS => {
                    attempts += 1;
                }
                Err(e) => {
                    return Err(AppError::internal(format!(
                        "create {}: {e}",
                        path.display()
                    )));
                }
            }
        };
        let tmp = Self {
            path,
            published: false,
        };
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| AppError::internal(format!("write {}: {e}", tmp.path.display())))?;
        Ok(tmp)
    }

    /// Renamed onto the target: nothing is left to remove.
    fn consumed(mut self) {
        self.published = true;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.published {
            // After a hard link this drops the temp name only; the published name keeps the data.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use wolluf_core::ErrorCode;

    use super::*;

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    struct Songs {
        _dir: tempfile::TempDir,
        songs: PathBuf,
    }

    fn songs() -> Songs {
        let dir = tempfile::tempdir().unwrap();
        let songs = dir.path().join("osu!").join("Songs");
        std::fs::create_dir_all(songs.join("100 set")).unwrap();
        std::fs::write(songs.join("100 set").join("map.osu"), b"osu").unwrap();
        Songs { _dir: dir, songs }
    }

    fn permit(folder: SetFolder, files: &[&str]) -> ExportPermit {
        let registry = PreviewRegistry::new(ExportParams::default());
        let id = registry.record(
            T0,
            Preview {
                folder,
                files: files.iter().map(|f| (*f).to_owned()).collect(),
                payload: (),
            },
        );
        confirm(&registry, &id, T0).unwrap().0
    }

    #[test]
    fn resolve_finds_the_set_folder_of_a_catalog_path() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        assert_eq!(
            folder.path(),
            std::fs::canonicalize(s.songs.join("100 set")).unwrap()
        );
        assert_eq!(folder.shown(), s.songs.join("100 set"));
    }

    #[test]
    fn resolve_rejects_paths_that_leave_songs() {
        let s = songs();
        let outside = s.songs.parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        for rel in [
            "../outside/map.osu",
            "100 set/../../outside/map.osu",
            "map.osu",
            "./map.osu",
            "",
        ] {
            let err = SetFolder::resolve(&s.songs, Path::new(rel)).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{rel:?}");
            assert_eq!(err.message_key, keys::FOLDER_OUTSIDE_SONGS, "{rel:?}");
        }
        let abs = outside.join("map.osu");
        assert!(SetFolder::resolve(&s.songs, &abs).is_err());
        assert_eq!(
            SetFolder::resolve(&s.songs, Path::new("gone/map.osu"))
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_rejects_a_symlinked_folder_pointing_outside_songs() {
        let s = songs();
        let outside = s.songs.parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, s.songs.join("200 link")).unwrap();
        let err = SetFolder::resolve(&s.songs, Path::new("200 link/map.osu")).unwrap_err();
        assert_eq!(err.message_key, keys::FOLDER_OUTSIDE_SONGS);
    }

    #[test]
    fn entry_accepts_one_plain_name_only() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        assert_eq!(
            folder.entry("a 1.15x.ogg").unwrap(),
            folder.path().join("a 1.15x.ogg")
        );
        for bad in [
            "", " ", ".", "..", "../x.ogg", "a/b.ogg", "a\\b.ogg", "a\0.ogg",
        ] {
            assert_eq!(
                folder.entry(bad).unwrap_err().code,
                ErrorCode::InvalidInput,
                "{bad:?}"
            );
        }
        assert!(folder.has_file("map.osu").unwrap());
        assert!(!folder.has_file("other.osu").unwrap());
    }

    #[test]
    fn entry_rejects_names_windows_cannot_hold() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        for bad in [
            "a:b.ogg", "c:x.ogg", "a<b.ogg", "a>b.ogg", "a\"b.ogg", "a|b.ogg", "a?b.ogg", "a*b.ogg",
        ] {
            assert_eq!(
                folder.entry(bad).unwrap_err().code,
                ErrorCode::InvalidInput,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn entry_caps_the_name_and_the_full_path() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let too_long_name = format!("{}.ogg", "é".repeat(126));
        assert_eq!(too_long_name.len(), 256);
        assert!(folder.entry(&too_long_name).is_err());

        // Measured on the path stable opens, separator included.
        let base = folder.shown().as_os_str().len() + 1;
        let fits = format!("{}.ogg", "a".repeat(MAX_PATH_UNITS - base - 4));
        assert!(
            fits.len() <= MAX_NAME_BYTES,
            "temp dir too deep for this test"
        );
        assert_eq!(folder.shown().join(&fits).as_os_str().len(), MAX_PATH_UNITS);
        assert!(folder.entry(&fits).is_ok());
        let over = format!("a{fits}");
        assert_eq!(
            folder.entry(&over).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }

    fn temp_files(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(TEMP_PREFIX))
            .collect()
    }

    fn written_path(outcome: &WriteOutcome) -> &Path {
        match outcome {
            WriteOutcome::Written(published) => published.path(),
            WriteOutcome::Exists(path) => panic!("expected a write, found {}", path.display()),
        }
    }

    #[test]
    fn write_new_creates_and_never_overwrites() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let permit = permit(folder.clone(), &["new.osu", "map.osu"]);
        let target = folder.path().join("new.osu");
        let outcome = write_new(&permit, "new.osu", b"one").unwrap();
        assert_eq!(written_path(&outcome), target);
        assert_eq!(std::fs::read(&target).unwrap(), b"one");
        assert_eq!(
            write_new(&permit, "new.osu", b"two").unwrap(),
            WriteOutcome::Exists(target.clone())
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"one");
        assert_eq!(
            write_new(&permit, "map.osu", b"clobber").unwrap(),
            WriteOutcome::Exists(folder.path().join("map.osu"))
        );
        assert_eq!(
            std::fs::read(folder.path().join("map.osu")).unwrap(),
            b"osu"
        );
        assert!(temp_files(folder.path()).is_empty(), "temp files left");
    }

    #[test]
    fn write_new_fails_on_a_target_that_is_not_a_file() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        std::fs::create_dir(folder.path().join("dir.osu")).unwrap();
        let permit = permit(folder.clone(), &["dir.osu"]);
        let err = write_new(&permit, "dir.osu", b"x").unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(err.message_key, keys::TARGET_NOT_A_FILE);
        assert!(folder.path().join("dir.osu").is_dir());
        assert!(temp_files(folder.path()).is_empty(), "temp files left");
    }

    #[test]
    fn publish_falls_back_to_a_reserved_rename_without_hard_links() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let no_links = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::Unsupported));

        let target = folder.path().join("new.osu");
        let tmp = TempFile::create(folder.path(), b"one").unwrap();
        let outcome = publish(tmp, &target, no_links).unwrap();
        assert_eq!(written_path(&outcome), target);
        assert_eq!(std::fs::read(&target).unwrap(), b"one");

        let tmp = TempFile::create(folder.path(), b"two").unwrap();
        assert_eq!(
            publish(tmp, &target, no_links).unwrap(),
            WriteOutcome::Exists(target.clone())
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"one");

        std::fs::create_dir(folder.path().join("dir.osu")).unwrap();
        let tmp = TempFile::create(folder.path(), b"three").unwrap();
        let err = publish(tmp, &folder.path().join("dir.osu"), no_links).unwrap_err();
        assert_eq!(err.message_key, keys::TARGET_NOT_A_FILE);
        assert!(temp_files(folder.path()).is_empty(), "temp files left");
    }

    #[test]
    fn retract_removes_a_file_this_permit_published() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let permit = permit(folder.clone(), &["new.ogg"]);
        let WriteOutcome::Written(published) = write_new(&permit, "new.ogg", b"ogg").unwrap()
        else {
            panic!("not written");
        };
        retract(&permit, published).unwrap();
        assert!(!folder.path().join("new.ogg").exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_new_refuses_a_folder_swapped_for_a_link() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let permit = permit(folder.clone(), &["new.osu"]);
        let outside = s.songs.parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::rename(s.songs.join("100 set"), s.songs.join("100 moved")).unwrap();
        std::os::unix::fs::symlink(&outside, s.songs.join("100 set")).unwrap();
        let err = write_new(&permit, "new.osu", b"x").unwrap_err();
        assert_eq!(err.message_key, keys::FOLDER_OUTSIDE_SONGS);
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        assert!(!s.songs.join("100 moved").join("new.osu").exists());
    }

    #[test]
    fn write_new_only_writes_the_files_the_preview_named() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let permit = permit(folder.clone(), &["new.osu"]);
        let err = write_new(&permit, "other.osu", b"x").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(!folder.path().join("other.osu").exists());
        let err = write_new(&permit, "../escape.osu", b"x").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(!s.songs.join("escape.osu").exists());
    }

    #[test]
    fn confirm_needs_a_recorded_live_preview_and_consumes_it() {
        let s = songs();
        let folder = SetFolder::resolve(&s.songs, Path::new("100 set/map.osu")).unwrap();
        let registry = PreviewRegistry::new(ExportParams {
            preview_ttl_us: 1_000,
            max_previews: 2,
        });
        let err = confirm(&registry, "01JNOPREVIEW", T0).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message_key, keys::PREVIEW_UNKNOWN);

        let preview = |payload: u8| Preview {
            folder: folder.clone(),
            files: vec!["new.osu".to_owned()],
            payload,
        };
        let id = registry.record(T0, preview(7));
        let (permit, payload) = confirm(&registry, &id, UnixUs(T0.0 + 1_000)).unwrap();
        assert_eq!((permit.preview_id(), payload), (id.as_str(), 7));
        assert_eq!(permit.folder(), &folder);
        assert_eq!(
            confirm(&registry, &id, T0).unwrap_err().code,
            ErrorCode::NotFound,
            "a preview mints one permit"
        );

        let stale = registry.record(T0, preview(1));
        let err = confirm(&registry, &stale, UnixUs(T0.0 + 1_001)).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound, "expired");

        let ids: Vec<String> = (0..3).map(|i| registry.record(T0, preview(i))).collect();
        assert!(confirm(&registry, &ids[0], T0).is_err(), "oldest evicted");
        assert!(confirm(&registry, &ids[2], T0).is_ok());
    }
}

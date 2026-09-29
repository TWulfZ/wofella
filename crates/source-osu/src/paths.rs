//! Pure Windows ↔ WSL path helpers.

use std::path::{Component, Path, PathBuf};

use crate::install::Platform;

/// Where WSL mounts Windows drives (drvfs).
pub const WSL_MOUNT_ROOT: &str = "/mnt";
const DEFAULT_SONGS_DIR: &str = "Songs";
const DRIVE_SEPARATOR: u8 = b':';

fn is_separator(b: u8) -> bool {
    b == b'\\' || b == b'/'
}

/// `X:\…` or `X:/…` (or bare `X:`); UNC and device paths (`\\server`, `\\?\`) are not.
pub fn is_windows_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == DRIVE_SEPARATOR
        && (b.len() == 2 || is_separator(b[2]))
}

/// Drive-letter paths only: `E:\Games\osu!` → `/mnt/e/Games/osu!`. UNC → `None` because WSL
/// has no stable mount point for network shares.
pub fn windows_to_wsl(path: &str) -> Option<PathBuf> {
    windows_to_wsl_under(path, Path::new(WSL_MOUNT_ROOT))
}

/// Same as [`windows_to_wsl`] with an injectable mount root, so tests can map drives into a
/// tempdir.
pub fn windows_to_wsl_under(path: &str, mount_root: &Path) -> Option<PathBuf> {
    if !is_windows_absolute(path) {
        return None;
    }
    let drive = char::from(path.as_bytes()[0].to_ascii_lowercase());
    let mut out = mount_root.join(drive.to_string());
    path[2..]
        .split(['\\', '/'])
        .filter(|seg| !seg.is_empty())
        .for_each(|seg| out.push(seg));
    Some(out)
}

/// `/mnt/<a-z>` or below: WSL drvfs, where inotify misses writes by Windows processes
/// (003 picks poll mode) and names are case-insensitive.
pub fn is_drvfs_path(path: &Path) -> bool {
    let mut parts = path.components();
    let is_drive = |c: Option<Component<'_>>| {
        c.and_then(|c| c.as_os_str().to_str())
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_lowercase())
    };
    parts.next() == Some(Component::RootDir)
        && parts.next().and_then(|c| c.as_os_str().to_str()) == Some("mnt")
        && is_drive(parts.next())
}

/// Joins `rel` onto a Windows-style base with `\`, so Windows paths come out identical on any
/// host (the pure candidate tests run on Linux).
pub(crate) fn windows_join(base: &str, rel: &[&str]) -> PathBuf {
    let mut out = base.trim_end_matches(['\\', '/']).to_owned();
    for seg in rel {
        out.push('\\');
        out.push_str(seg);
    }
    PathBuf::from(out)
}

/// cfg `BeatmapDirectory`: relative → under the root; absolute Windows → translated on WSL;
/// empty, missing or unusable on this platform → `<root>/Songs`.
pub fn resolve_songs_dir(
    root: &Path,
    beatmap_directory: Option<&str>,
    platform: Platform,
) -> PathBuf {
    let default = || join_under(root, &[DEFAULT_SONGS_DIR], platform);
    let Some(dir) = beatmap_directory.map(str::trim).filter(|d| !d.is_empty()) else {
        return default();
    };
    if is_windows_absolute(dir) {
        return match platform {
            Platform::Windows => PathBuf::from(dir),
            Platform::Wsl => windows_to_wsl(dir).unwrap_or_else(default),
            Platform::Linux | Platform::Other => default(),
        };
    }
    let segments: Vec<&str> = dir.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    join_under(root, &segments, platform)
}

pub(crate) fn join_under(root: &Path, rel: &[&str], platform: Platform) -> PathBuf {
    match platform {
        Platform::Windows => windows_join(&root.to_string_lossy(), rel),
        _ => rel
            .iter()
            .fold(root.to_path_buf(), |acc, seg| acc.join(seg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_to_wsl_drive_path() {
        assert_eq!(
            windows_to_wsl(r"E:\Games\osu!"),
            Some(PathBuf::from("/mnt/e/Games/osu!"))
        );
        assert_eq!(
            windows_to_wsl(r"c:/Users/x/"),
            Some(PathBuf::from("/mnt/c/Users/x"))
        );
        assert_eq!(windows_to_wsl("D:"), Some(PathBuf::from("/mnt/d")));
        assert_eq!(windows_to_wsl(r"D:\"), Some(PathBuf::from("/mnt/d")));
        assert_eq!(
            windows_to_wsl_under(r"E:\osu!", Path::new("/tmp/fake")),
            Some(PathBuf::from("/tmp/fake/e/osu!"))
        );
        assert_eq!(windows_to_wsl("Songs"), None);
        assert_eq!(windows_to_wsl(r"E:relative"), None);
        assert_eq!(windows_to_wsl("/mnt/e/x"), None);
        assert_eq!(windows_to_wsl(r"1:\x"), None);
    }

    #[test]
    fn unc_not_translated() {
        assert_eq!(windows_to_wsl(r"\\server\share\osu!"), None);
        assert_eq!(windows_to_wsl(r"\\?\C:\osu!"), None);
        assert!(!is_windows_absolute(r"\\server\share"));
    }

    #[test]
    fn drvfs_detection() {
        assert!(is_drvfs_path(Path::new("/mnt/e/Games/osu!")));
        assert!(is_drvfs_path(Path::new("/mnt/c")));
        assert!(!is_drvfs_path(Path::new("/mnt/wsl/x")));
        assert!(!is_drvfs_path(Path::new("/mnt/E/x")));
        assert!(!is_drvfs_path(Path::new("/home/u/osu!")));
        assert!(!is_drvfs_path(Path::new("mnt/e/x")));
        assert!(!is_drvfs_path(Path::new("/mnt")));
    }

    #[test]
    fn songs_dir_relative_absolute_default() {
        let root = Path::new("/mnt/e/Games/osu!");
        assert_eq!(
            resolve_songs_dir(root, None, Platform::Wsl),
            root.join("Songs")
        );
        assert_eq!(
            resolve_songs_dir(root, Some(""), Platform::Wsl),
            root.join("Songs")
        );
        assert_eq!(
            resolve_songs_dir(root, Some(r"Songs\7k"), Platform::Wsl),
            root.join("Songs/7k")
        );
        assert_eq!(
            resolve_songs_dir(root, Some(r"D:\osu songs"), Platform::Wsl),
            PathBuf::from("/mnt/d/osu songs")
        );
        // A Windows drive path is meaningless on native Linux.
        assert_eq!(
            resolve_songs_dir(root, Some(r"D:\osu songs"), Platform::Linux),
            root.join("Songs")
        );
        let win_root = Path::new(r"E:\Games\osu!");
        assert_eq!(
            resolve_songs_dir(win_root, Some(r"D:\osu songs"), Platform::Windows),
            PathBuf::from(r"D:\osu songs")
        );
        assert_eq!(
            resolve_songs_dir(win_root, Some("Songs"), Platform::Windows),
            PathBuf::from(r"E:\Games\osu!\Songs")
        );
        assert_eq!(
            resolve_songs_dir(win_root, None, Platform::Windows),
            PathBuf::from(r"E:\Games\osu!\Songs")
        );
    }
}

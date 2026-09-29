//! Discovery and reading of `osu!.<account>.cfg` files. `<account>` is the Windows account
//! name, not an osu! name (pilot: `twulfz`); `osu!.cfg` is the global config and is skipped.

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::codec::cfg::{UserCfg, parse_user_cfg};
use crate::diag::Diagnostics;
use crate::error::SourceError;

const CFG_PREFIX: &str = "osu!.";
const CFG_SUFFIX: &str = ".cfg";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgFile {
    pub path: PathBuf,
    pub account: String,
    pub mtime: SystemTime,
}

/// Windows file names are case-insensitive, so the pattern is matched that way too.
fn account_of(file_name: &str) -> Option<&str> {
    let prefix = file_name.get(..CFG_PREFIX.len())?;
    let suffix_at = file_name.len().checked_sub(CFG_SUFFIX.len())?;
    let suffix = file_name.get(suffix_at..)?;
    if !prefix.eq_ignore_ascii_case(CFG_PREFIX) || !suffix.eq_ignore_ascii_case(CFG_SUFFIX) {
        return None;
    }
    let account = file_name.get(CFG_PREFIX.len()..suffix_at)?;
    (!account.is_empty()).then_some(account)
}

/// Newest mtime first (004's session user and the songs dir both use the newest file, spec 002
/// R-d); ties break by path so the order is deterministic.
pub fn list_user_cfgs(root: &Path) -> Result<Vec<CfgFile>, SourceError> {
    let entries = fs::read_dir(root).map_err(|e| SourceError::io(root, &e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| SourceError::io(root, &e))?;
        let name = entry.file_name();
        let Some(account) = name.to_str().and_then(account_of) else {
            continue;
        };
        let path = entry.path();
        let meta = fs::metadata(&path).map_err(|e| SourceError::io(&path, &e))?;
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().map_err(|e| SourceError::io(&path, &e))?;
        out.push(CfgFile {
            account: account.to_owned(),
            path,
            mtime,
        });
    }
    out.sort_by(|a, b| (Reverse(a.mtime), &a.path).cmp(&(Reverse(b.mtime), &b.path)));
    Ok(out)
}

/// The buffer is dropped on return: the cfg is never vaulted or hashed as a whole.
pub fn read_user_cfg(path: &Path) -> Result<(UserCfg, Diagnostics), SourceError> {
    let bytes = fs::read(path).map_err(|e| SourceError::io(path, &e))?;
    Ok(parse_user_cfg(&bytes))
}

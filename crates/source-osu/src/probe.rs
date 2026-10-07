//! Is osu! running? (spec 002 "osu!-running probe"; D8 names this trait). Callers must treat
//! `Unknown` as "may be running"; this crate does not decide policy. The same probe reads
//! stable's window title, which names the chart being played.

use std::process::Command;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::install::{DetectEnv, Platform, current_platform};
use crate::process::{DEFAULT_TIMEOUT, run_with_timeout};

const OSU_IMAGE: &str = "osu!.exe";
/// Native Linux/Wine may report the name without the extension.
const OSU_IMAGE_BARE: &str = "osu!";
const WSL_TASKLIST_EXE: &str = "/mnt/c/Windows/System32/tasklist.exe";
#[cfg(any(windows, test))]
const DEFAULT_SYSTEM_ROOT: &str = "C:\\Windows";
const CSV_QUOTE: char = '"';
const CSV_SEPARATOR: char = ',';
/// `tasklist /V` columns: image, PID, session, session#, memory, status, user, CPU time, title.
const VERBOSE_TITLE_COLUMN: usize = 8;
/// Stable's title prefix; one or two spaces separate it from `- Artist - Title [Difficulty]`.
const OSU_TITLE_PREFIX: &str = "osu!";
const OSU_TITLE_MAX_SPACES: usize = 2;
const OSU_TITLE_SEPARATOR: &str = "- ";
const ARTIST_TITLE_SEPARATOR: &str = " - ";
const VERSION_OPEN: &str = " [";
const VERSION_CLOSE: char = ']';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    /// Pids ascending.
    Running {
        pids: Vec<u32>,
    },
    NotRunning,
    Unknown {
        reason: String,
    },
}

impl ProbeResult {
    /// The F4 export gate treats `Unknown` as running.
    pub const fn may_be_running(&self) -> bool {
        !matches!(self, Self::NotRunning)
    }
}

pub trait OsuProcessProbe: Send + Sync {
    fn probe(&self) -> ProbeResult;
    fn name(&self) -> &'static str;

    /// osu!'s main window title, `None` when osu! is not running or the platform cannot tell.
    /// Spawns a process: call it on demand, never on a timer (D9).
    fn window_title(&self) -> Option<String> {
        None
    }
}

/// The chart named by stable's window title, as `Artist - Title [Difficulty]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingTitle {
    pub artist_title_version: String,
}

/// Operational limits, not algorithm thresholds (spec 002 Design).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeParams {
    pub timeout: Duration,
}

impl Default for ProbeParams {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// Windows and native Linux. `sysinfo` because the workspace denies `unsafe_code`, which rules
/// out calling ToolHelp32 ourselves; enumerating all processes costs ≈ 10–50 ms, so callers
/// probe on demand (spec 002 R-c).
#[derive(Debug, Default, Clone, Copy)]
pub struct SysinfoProbe;

impl OsuProcessProbe for SysinfoProbe {
    fn probe(&self) -> ProbeResult {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        let mut pids: Vec<u32> = system
            .processes()
            .values()
            .filter(|p| {
                let name = p.name().to_string_lossy();
                name.eq_ignore_ascii_case(OSU_IMAGE) || name.eq_ignore_ascii_case(OSU_IMAGE_BARE)
            })
            .map(|p| p.pid().as_u32())
            .collect();
        pids.sort_unstable();
        if pids.is_empty() {
            ProbeResult::NotRunning
        } else {
            ProbeResult::Running { pids }
        }
    }

    fn name(&self) -> &'static str {
        "sysinfo"
    }

    fn window_title(&self) -> Option<String> {
        #[cfg(windows)]
        {
            let root = std::env::var_os("SystemRoot");
            tasklist_window_title(windows_tasklist_exe(root.as_deref()), DEFAULT_TIMEOUT)
        }
        // Native Linux runs stable under Wine, whose window titles are out of scope.
        #[cfg(not(windows))]
        {
            None
        }
    }
}

/// WSL cannot see Windows processes directly; `tasklist.exe` runs through interop and is killed
/// at the deadline (spec 002 R-f: interop disabled → `Unknown`).
#[derive(Debug, Default, Clone, Copy)]
pub struct WslTasklistProbe {
    pub params: ProbeParams,
}

impl OsuProcessProbe for WslTasklistProbe {
    fn probe(&self) -> ProbeResult {
        let mut cmd = Command::new(WSL_TASKLIST_EXE);
        cmd.args([
            "/FI",
            &format!("IMAGENAME eq {OSU_IMAGE}"),
            "/FO",
            "CSV",
            "/NH",
        ]);
        match run_with_timeout(cmd, self.params.timeout) {
            Ok(out) => parse_tasklist_csv(&String::from_utf8_lossy(&out)),
            Err(e) => ProbeResult::Unknown {
                reason: format!("tasklist.exe: {e}"),
            },
        }
    }

    fn name(&self) -> &'static str {
        "wsl_tasklist"
    }

    fn window_title(&self) -> Option<String> {
        tasklist_window_title(WSL_TASKLIST_EXE, self.params.timeout)
    }
}

/// An absolute path: a bare `tasklist` would resolve through the current directory and `PATH`
/// first, so a planted `tasklist.exe` next to the app would run instead.
#[cfg(any(windows, test))]
fn windows_tasklist_exe(system_root: Option<&std::ffi::OsStr>) -> std::path::PathBuf {
    let root = system_root
        .filter(|r| !r.is_empty())
        .unwrap_or(std::ffi::OsStr::new(DEFAULT_SYSTEM_ROOT));
    std::path::Path::new(root)
        .join("System32")
        .join("tasklist.exe")
}

fn tasklist_window_title(exe: impl AsRef<std::ffi::OsStr>, timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.args([
        "/V",
        "/FI",
        &format!("IMAGENAME eq {OSU_IMAGE}"),
        "/FO",
        "CSV",
        "/NH",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a GUI app spawning a console tool would otherwise flash a window.
        cmd.creation_flags(0x0800_0000);
    }
    match run_with_timeout(cmd, timeout) {
        Ok(out) => parse_tasklist_window_title(&String::from_utf8_lossy(&out)),
        Err(e) => {
            tracing::debug!(error = %e, "tasklist window title");
            None
        }
    }
}

/// Output of `tasklist /FI "IMAGENAME eq osu!.exe" /FO CSV /NH`. With no match tasklist prints a
/// single localised `INFO:` line (OEM code page), so any non-CSV line with a colon counts as
/// that message; anything else is `Unknown`.
pub fn parse_tasklist_csv(output: &str) -> ProbeResult {
    let mut pids = Vec::new();
    let mut info = false;
    for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !line.starts_with(CSV_QUOTE) {
            if line.contains(':') {
                info = true;
                continue;
            }
            return unknown("unrecognised line");
        }
        let mut fields = line.split(',').map(|f| f.trim_matches(CSV_QUOTE));
        let (Some(image), Some(pid)) = (fields.next(), fields.next()) else {
            return unknown("short CSV row");
        };
        if !image.eq_ignore_ascii_case(OSU_IMAGE) {
            return unknown("unexpected image in filtered output");
        }
        let Ok(pid) = pid.parse::<u32>() else {
            return unknown("pid is not a number");
        };
        pids.push(pid);
    }
    pids.sort_unstable();
    match (pids.is_empty(), info) {
        (false, _) => ProbeResult::Running { pids },
        (true, true) => ProbeResult::NotRunning,
        (true, false) => unknown("empty output"),
    }
}

/// The "Window Title" of the first `osu!.exe` row of `tasklist /V /FI "IMAGENAME eq osu!.exe"
/// /FO CSV /NH`. Titles may hold commas and doubled quotes, so rows go through a real CSV split.
/// tasklist writes the OEM code page, so non-ASCII titles arrive as replacement characters.
pub fn parse_tasklist_window_title(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with(CSV_QUOTE))
        .filter_map(csv_fields)
        .filter(|f| f.first().is_some_and(|i| i.eq_ignore_ascii_case(OSU_IMAGE)))
        .find_map(|mut f| {
            (f.len() > VERBOSE_TITLE_COLUMN).then(|| f.swap_remove(VERBOSE_TITLE_COLUMN))
        })
}

/// One CSV line; `None` for an unterminated quote or text after a closing quote.
fn csv_fields(line: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        let mut field = String::new();
        if chars.next_if_eq(&CSV_QUOTE).is_some() {
            loop {
                match chars.next()? {
                    CSV_QUOTE if chars.next_if_eq(&CSV_QUOTE).is_some() => field.push(CSV_QUOTE),
                    CSV_QUOTE => break,
                    c => field.push(c),
                }
            }
        } else {
            while let Some(c) = chars.next_if(|&c| c != CSV_SEPARATOR) {
                field.push(c);
            }
        }
        fields.push(field);
        match chars.next() {
            None => return Some(fields),
            Some(CSV_SEPARATOR) => {}
            Some(_) => return None,
        }
    }
}

/// Stable shows `osu!` when idle and `osu!  - Artist - Title [Difficulty]` while playing (two
/// spaces; one is accepted too). Every other shape is `None`: the editor's `….osu` file name,
/// cutting-edge builds, spectating, other prefixes.
pub fn parse_osu_title(title: &str) -> Option<NowPlayingTitle> {
    let rest = title.trim_end().strip_prefix(OSU_TITLE_PREFIX)?;
    let after_spaces = rest.trim_start_matches(' ');
    let spaces = rest.len() - after_spaces.len();
    if !(1..=OSU_TITLE_MAX_SPACES).contains(&spaces) {
        return None;
    }
    let chart = after_spaces.strip_prefix(OSU_TITLE_SEPARATOR)?.trim();
    let open = chart.find(VERSION_OPEN)?;
    if !chart.ends_with(VERSION_CLOSE) || !chart[..open].contains(ARTIST_TITLE_SEPARATOR) {
        return None;
    }
    Some(NowPlayingTitle {
        artist_title_version: chart.to_owned(),
    })
}

fn unknown(reason: &str) -> ProbeResult {
    ProbeResult::Unknown {
        reason: reason.to_owned(),
    }
}

pub fn default_probe(env: &DetectEnv, params: ProbeParams) -> Box<dyn OsuProcessProbe> {
    match env.platform {
        Platform::Wsl => Box::new(WslTasklistProbe { params }),
        Platform::Windows | Platform::Linux | Platform::Other => Box::new(SysinfoProbe),
    }
}

/// [`default_probe`] for this host without a full [`DetectEnv`], which on WSL spawns `reg.exe`.
pub fn system_probe(params: ProbeParams) -> Box<dyn OsuProcessProbe> {
    default_probe(
        &DetectEnv {
            platform: current_platform(),
            ..DetectEnv::default()
        },
        params,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::FakeProbe;

    #[test]
    fn tasklist_csv_running() {
        let out = "\r\n\"osu!.exe\",\"12345\",\"Console\",\"1\",\"250,000 K\"\r\n\"OSU!.EXE\",\"7\",\"Console\",\"1\",\"1 K\"\r\n";
        assert_eq!(
            parse_tasklist_csv(out),
            ProbeResult::Running {
                pids: vec![7, 12345]
            }
        );
    }

    #[test]
    fn tasklist_csv_info_line_not_running() {
        let out = "INFO: No tasks are running which match the specified criteria.\r\n";
        assert_eq!(parse_tasklist_csv(out), ProbeResult::NotRunning);
        // Localised Windows prints the same line in the OEM code page.
        let es = "INFORMACI\u{fffd}N: no hay tareas ejecut\u{fffd}ndose que coincidan con los criterios especificados.\r\n";
        assert_eq!(parse_tasklist_csv(es), ProbeResult::NotRunning);
    }

    #[test]
    fn tasklist_garbage_unknown() {
        for out in [
            "",
            "\r\n",
            "garbage without colon",
            "\"osu!.exe\",\"notapid\"",
            "\"osu!.exe\"",
        ] {
            assert!(
                matches!(parse_tasklist_csv(out), ProbeResult::Unknown { .. }),
                "{out:?}"
            );
        }
        // Another image in the output means the filter did not apply: do not trust it.
        assert!(matches!(
            parse_tasklist_csv("\"explorer.exe\",\"1\",\"Console\",\"1\",\"1 K\"\r\n"),
            ProbeResult::Unknown { .. }
        ));
    }

    #[test]
    fn fake_probe_reports_configured_state() {
        let probes: Vec<Box<dyn OsuProcessProbe>> = vec![
            Box::new(FakeProbe::new(ProbeResult::NotRunning)),
            Box::new(FakeProbe::new(ProbeResult::Running { pids: vec![4] })),
        ];
        assert_eq!(probes[0].probe(), ProbeResult::NotRunning);
        assert_eq!(probes[1].probe(), ProbeResult::Running { pids: vec![4] });
        let fake = FakeProbe::new(ProbeResult::NotRunning);
        fake.set(ProbeResult::Unknown {
            reason: "timeout".into(),
        });
        assert!(fake.probe().may_be_running());
        assert!(!ProbeResult::NotRunning.may_be_running());
    }

    #[test]
    fn osu_title_while_playing_gives_the_chart() {
        let chart = |t: &str| parse_osu_title(t).map(|n| n.artist_title_version);
        for title in [
            "osu!  - Camellia - Exit This Earth's Atomosphere [7K Insane]",
            "osu! - Camellia - Exit This Earth's Atomosphere [7K Insane]",
            "osu!  - Camellia - Exit This Earth's Atomosphere [7K Insane]  ",
        ] {
            assert_eq!(
                chart(title).as_deref(),
                Some("Camellia - Exit This Earth's Atomosphere [7K Insane]"),
                "{title:?}"
            );
        }
        assert_eq!(
            chart("osu!  - A - B - C [Hard [7K]]").as_deref(),
            Some("A - B - C [Hard [7K]]"),
            "dashes and brackets inside the names stay"
        );
    }

    #[test]
    fn osu_title_otherwise_is_none() {
        for title in [
            "",
            "N/A",
            "osu!",
            "osu!  ",
            "osu!  - ",
            "osu!   - A - B [C]",
            "osu!- A - B [C]",
            "osu!cuttingedge b20260924  - A - B [C]",
            "A - B [C]",
            "osu!  - A - B (mapper) [C].osu",
            "osu!  - A - B",
            "osu!  - [C]",
        ] {
            assert_eq!(parse_osu_title(title), None, "{title:?}");
        }
    }

    #[test]
    fn verbose_tasklist_csv_gives_the_window_title() {
        let row = "\"osu!.exe\",\"12345\",\"Console\",\"1\",\"250,000 K\",\"Running\",\"PC\\u\",\"0:01:02\",\"osu!  - A, \"\"B\"\" - C [D]\"\r\n";
        assert_eq!(
            parse_tasklist_window_title(row).as_deref(),
            Some("osu!  - A, \"B\" - C [D]")
        );
        let idle = "\"osu!.exe\",\"1\",\"Console\",\"1\",\"1 K\",\"Running\",\"PC\\u\",\"0:00:01\",\"osu!\"";
        assert_eq!(parse_tasklist_window_title(idle).as_deref(), Some("osu!"));
    }

    #[test]
    fn verbose_tasklist_without_an_osu_row_has_no_title() {
        for out in [
            "",
            "INFO: No tasks are running which match the specified criteria.\r\n",
            "\"osu!.exe\",\"1\",\"Console\"",
            "\"explorer.exe\",\"1\",\"Console\",\"1\",\"1 K\",\"Running\",\"u\",\"0:00:01\",\"osu!  - A - B [C]\"",
            "\"osu!.exe\",\"1\",\"Console\",\"1\",\"1 K\",\"Running\",\"u\",\"0:00:01\",\"unterminated",
        ] {
            assert_eq!(parse_tasklist_window_title(out), None, "{out:?}");
        }
    }

    #[test]
    fn fake_probe_reports_a_configured_title() {
        let fake = FakeProbe::new(ProbeResult::NotRunning);
        assert_eq!(fake.window_title(), None);
        let fake = fake.with_title("osu!  - A - B [C]");
        assert_eq!(fake.window_title().as_deref(), Some("osu!  - A - B [C]"));
        // Native Linux has no stable window to read; Wine titles are out of scope.
        #[cfg(not(windows))]
        assert_eq!(SysinfoProbe.window_title(), None);
    }

    #[test]
    fn windows_tasklist_is_spawned_from_system_root() {
        use std::ffi::OsStr;
        use std::path::Path;
        let under = |root: &str| Path::new(root).join("System32").join("tasklist.exe");
        assert_eq!(
            windows_tasklist_exe(Some(OsStr::new("D:\\Win"))),
            under("D:\\Win")
        );
        for unset in [None, Some(OsStr::new(""))] {
            assert_eq!(
                windows_tasklist_exe(unset),
                under("C:\\Windows"),
                "{unset:?}"
            );
        }
    }

    #[test]
    fn sysinfo_probe_gives_a_definite_answer() {
        assert!(!matches!(SysinfoProbe.probe(), ProbeResult::Unknown { .. }));
    }

    #[test]
    fn default_probe_picks_by_platform() {
        use crate::install::{DetectEnv, Platform};
        let wsl = DetectEnv {
            platform: Platform::Wsl,
            ..DetectEnv::default()
        };
        assert_eq!(
            default_probe(&wsl, ProbeParams::default()).name(),
            "wsl_tasklist"
        );
        let linux = DetectEnv {
            platform: Platform::Linux,
            ..DetectEnv::default()
        };
        assert_eq!(
            default_probe(&linux, ProbeParams::default()).name(),
            "sysinfo"
        );
    }
}

//! Is osu! running? (spec 002 "osu!-running probe"; D8 names this trait). Callers must treat
//! `Unknown` as "may be running"; this crate does not decide policy.

use std::process::Command;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::install::{DetectEnv, Platform};
use crate::process::{DEFAULT_TIMEOUT, run_with_timeout};

const OSU_IMAGE: &str = "osu!.exe";
/// Native Linux/Wine may report the name without the extension.
const OSU_IMAGE_BARE: &str = "osu!";
const WSL_TASKLIST_EXE: &str = "/mnt/c/Windows/System32/tasklist.exe";
const CSV_QUOTE: char = '"';

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

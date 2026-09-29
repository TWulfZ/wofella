//! Child processes with a hard deadline (WSL interop `reg.exe` and `tasklist.exe`). A hung
//! interop call must not stall detection, so the child is killed at the deadline.

use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Spec 002: `ProbeParams.timeout` default.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, thiserror::Error)]
pub(crate) enum RunError {
    #[error("spawn failed: {0}")]
    Spawn(io::Error),
    #[error("wait failed: {0}")]
    Wait(io::Error),
    #[error("timed out")]
    Timeout,
    #[error("exited unsuccessfully")]
    Failed,
}

/// Returns stdout of a successful exit. Stdout is drained on a thread so a chatty child cannot
/// block on a full pipe while we poll.
pub(crate) fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<Vec<u8>, RunError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(RunError::Spawn)?;
    let stdout = child.stdout.take();
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout {
            // A read error only truncates the output, which the caller's parser then rejects.
            let _ = out.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                // Best effort: the child may have exited between try_wait and kill.
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(e) => return Err(RunError::Wait(e)),
        }
    };
    let out = reader.join().map_err(|_| RunError::Failed)?;
    if status.success() {
        Ok(out)
    } else {
        Err(RunError::Failed)
    }
}

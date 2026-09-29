use std::sync::{Mutex, PoisonError};

use crate::probe::{OsuProcessProbe, ProbeResult};

/// Reports whatever state the test configured; `set` changes it mid-test.
#[derive(Debug)]
pub struct FakeProbe {
    result: Mutex<ProbeResult>,
}

impl FakeProbe {
    pub const fn new(result: ProbeResult) -> Self {
        Self {
            result: Mutex::new(result),
        }
    }

    pub fn set(&self, result: ProbeResult) {
        *self.result.lock().unwrap_or_else(PoisonError::into_inner) = result;
    }
}

impl OsuProcessProbe for FakeProbe {
    fn probe(&self) -> ProbeResult {
        self.result
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn name(&self) -> &'static str {
        "fake"
    }
}

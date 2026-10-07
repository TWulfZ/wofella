use std::sync::{Mutex, PoisonError};

use crate::probe::{OsuProcessProbe, ProbeResult};

/// Reports whatever state the test configured; `set` changes it mid-test.
#[derive(Debug)]
pub struct FakeProbe {
    result: Mutex<ProbeResult>,
    title: Mutex<Option<String>>,
}

impl FakeProbe {
    pub const fn new(result: ProbeResult) -> Self {
        Self {
            result: Mutex::new(result),
            title: Mutex::new(None),
        }
    }

    pub fn with_title(self, title: impl Into<String>) -> Self {
        *self.title.lock().unwrap_or_else(PoisonError::into_inner) = Some(title.into());
        self
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

    fn window_title(&self) -> Option<String> {
        self.title
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

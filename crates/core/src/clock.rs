//! Wall-clock port (D8 b): domain code never reads the system time, it is handed a `Clock`.

use std::sync::atomic::{AtomicI64, Ordering};

use crate::time::UnixUs;

pub trait Clock: Send + Sync {
    fn now(&self) -> UnixUs;
}

/// Test clock shared by every crate; `SystemClock` lives in `wolluf-app` (spec 005).
#[derive(Debug)]
pub struct FixedClock(AtomicI64);

impl FixedClock {
    pub fn new(now: UnixUs) -> Self {
        Self(AtomicI64::new(now.0))
    }

    pub fn set(&self, now: UnixUs) {
        self.0.store(now.0, Ordering::SeqCst);
    }

    pub fn advance(&self, by_us: i64) {
        self.0.fetch_add(by_us, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> UnixUs {
        UnixUs(self.0.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_set_and_advance() {
        let clock = FixedClock::new(UnixUs(1_000));
        assert_eq!(clock.now(), UnixUs(1_000));
        clock.advance(500);
        assert_eq!(clock.now(), UnixUs(1_500));
        clock.set(UnixUs(-7));
        assert_eq!(clock.now(), UnixUs(-7));
        let shared: &dyn Clock = &clock;
        assert_eq!(shared.now(), UnixUs(-7));
    }
}

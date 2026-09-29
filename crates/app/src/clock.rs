//! The real wall clock, injected wherever a `wolluf_core::Clock` is needed (D8 b).

use std::time::{SystemTime, UNIX_EPOCH};

use wolluf_core::{Clock, UnixUs};

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    // The one allowed `SystemTime::now` site (docs/conventions.md, lint allowances).
    #[allow(clippy::disallowed_methods)]
    fn now(&self) -> UnixUs {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(since) => UnixUs(i64::try_from(since.as_micros()).unwrap_or(i64::MAX)),
            Err(before) => {
                UnixUs(i64::try_from(before.duration().as_micros()).map_or(i64::MIN, |us| -us))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-09-01T00:00:00Z, before this crate existed.
    const SEPT_2026_US: i64 = 1_788_220_800_000_000;

    #[test]
    fn system_clock_is_after_2026_09_01() {
        let clock = SystemClock;
        let first = clock.now();
        assert!(first > UnixUs(SEPT_2026_US), "{first:?}");
        assert!(clock.now() >= first);
    }
}

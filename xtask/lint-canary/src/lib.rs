//! One deliberate violation per lint that `cargo xtask lint-canary` expects to fire.
//! Removing any function or call here must make the canary fail: the runner expects every
//! disallowed path of the root clippy.toml to fire, because clippy 1.98 silently ignores an
//! unresolvable primitive path such as `f64::powfx` instead of warning.

use std::collections::{HashMap, HashSet};

pub fn unwrap_used(x: Option<u8>) -> u8 {
    x.unwrap()
}

pub fn expect_used(x: Option<u8>) -> u8 {
    x.expect("canary")
}

pub fn panic_used() {
    panic!("canary")
}

pub fn disallowed_hash_map() -> HashMap<u8, u8> {
    HashMap::new()
}

pub fn disallowed_hash_set() -> HashSet<u8> {
    HashSet::new()
}

pub fn disallowed_system_time_now() -> std::time::SystemTime {
    std::time::SystemTime::now()
}

// One call per `f64::*` entry of clippy.toml disallowed-methods.
pub fn disallowed_f64_transcendentals(x: f64) -> [f64; 26] {
    [
        x.exp(),
        x.exp2(),
        x.exp_m1(),
        x.ln(),
        x.ln_1p(),
        x.log(x),
        x.log2(),
        x.log10(),
        x.powf(x),
        x.powi(2),
        x.sin(),
        x.cos(),
        x.tan(),
        x.sin_cos().0,
        x.asin(),
        x.acos(),
        x.atan(),
        x.atan2(x),
        x.sinh(),
        x.cosh(),
        x.tanh(),
        x.asinh(),
        x.acosh(),
        x.atanh(),
        x.cbrt(),
        x.hypot(x),
    ]
}

// One call per `f32::*` entry of clippy.toml disallowed-methods.
pub fn disallowed_f32_transcendentals(x: f32) -> [f32; 26] {
    [
        x.exp(),
        x.exp2(),
        x.exp_m1(),
        x.ln(),
        x.ln_1p(),
        x.log(x),
        x.log2(),
        x.log10(),
        x.powf(x),
        x.powi(2),
        x.sin(),
        x.cos(),
        x.tan(),
        x.sin_cos().0,
        x.asin(),
        x.acos(),
        x.atan(),
        x.atan2(x),
        x.sinh(),
        x.cosh(),
        x.tanh(),
        x.asinh(),
        x.acosh(),
        x.atanh(),
        x.cbrt(),
        x.hypot(x),
    ]
}

//! Etterna MinaCalc v527 behind a C ABI (ADR 0022).
//!
//! Outputs are platform floats (libm, `rsqrtss`): quantise them before they reach a hash.

// ADR 0022: the only workspace crate allowed `unsafe`, confined to the FFI calls below.
#![allow(unsafe_code)]

use std::ffi::{c_int, c_uint, c_void};
use std::fmt;
use std::ptr::NonNull;

pub const CALC_VERSION: i32 = 527;

pub const SKILLSET_IDS: [&str; 8] = [
    "overall",
    "stream",
    "jumpstream",
    "handstream",
    "stamina",
    "jackspeed",
    "chordjack",
    "technical",
];

/// One chart row: bit `c` of `notes` is column `c` from the left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteRow {
    pub notes: u32,
    pub time_s: f32,
}

/// Values in [`SKILLSET_IDS`] order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Skillsets(pub [f32; 8]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcError {
    Empty,
    NotIncreasing { index: usize },
    MaskOutOfRange { index: usize },
    UnsupportedKeycount(u8),
    Native,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("no note rows"),
            Self::NotIncreasing { index } => write!(
                f,
                "row {index}: time is not finite, negative, or not after the previous row"
            ),
            Self::MaskOutOfRange { index } => write!(
                f,
                "row {index}: note mask is empty or names a column past the keycount"
            ),
            Self::UnsupportedKeycount(k) => write!(f, "MinaCalc does not support {k}K"),
            Self::Native => f.write_str("MinaCalc rejected the chart or failed"),
        }
    }
}

impl std::error::Error for CalcError {}

/// Upstream ships tuned logic for 4K–7K and a generic one up to 10K (Etterna's largest game);
/// below 4K its hand masks are degenerate.
const KEYCOUNTS: std::ops::RangeInclusive<u8> = 4..=10;

/// Upstream casts `time / rate / 0.5` to `int` before testing its 100000-interval cap,
/// which is UB for huge values, so the cap is enforced here first.
const MAX_SCALED_TIME_S: f32 = 100_000.0 * 0.5;

const STATUS_OK: c_int = 0;

unsafe extern "C" {
    fn wl_calc_new() -> *mut c_void;
    fn wl_calc_free(calc: *mut c_void);
    fn wl_calc_version() -> c_int;
    fn wl_msd(
        calc: *mut c_void,
        notes: *const u32,
        times: *const f32,
        n: usize,
        rate: f32,
        keycount: c_uint,
        out: *mut f32,
    ) -> c_int;
    fn wl_ssr(
        calc: *mut c_void,
        notes: *const u32,
        times: *const f32,
        n: usize,
        rate: f32,
        goal: f32,
        keycount: c_uint,
        out: *mut f32,
    ) -> c_int;
}

/// One native calculator. It keeps scratch buffers between calls, so use one per thread.
///
/// ```compile_fail
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<wolluf_minacalc::Calc>();
/// ```
#[derive(Debug)]
pub struct Calc {
    // NonNull keeps Calc !Sync: the native object mutates itself on every call.
    raw: NonNull<c_void>,
}

// SAFETY: the native Calc owns all its state and has no thread affinity; only shared
// access from two threads at once would race, and Calc is not Sync.
unsafe impl Send for Calc {}

impl Calc {
    pub fn new() -> Result<Self, CalcError> {
        // SAFETY: no arguments; a null return (allocation failure) is handled below.
        let raw = unsafe { wl_calc_new() };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or(CalcError::Native)
    }

    pub fn version() -> i32 {
        // SAFETY: reads a native global; takes no pointers.
        unsafe { wl_calc_version() }
    }

    /// Chart difficulty (MSD) at one rate, as Etterna caches it (score goal 0.93).
    pub fn msd(
        &mut self,
        rows: &[NoteRow],
        rate: f32,
        keycount: u8,
    ) -> Result<Skillsets, CalcError> {
        let input = Input::new(rows, rate, keycount)?;
        let mut out = [0.0f32; 8];
        // SAFETY: `raw` is live and exclusively borrowed; both slices hold `n` elements;
        // `out` holds the 8 floats the shim writes.
        let status = unsafe {
            wl_msd(
                self.raw.as_ptr(),
                input.notes.as_ptr(),
                input.times.as_ptr(),
                input.notes.len(),
                rate,
                c_uint::from(keycount),
                out.as_mut_ptr(),
            )
        };
        finish(status, out)
    }

    /// Score-based skill rating (SSR) for a score at `goal` (Wife%, 1.0 = 100%;
    /// upstream caps it at 0.965).
    pub fn ssr(
        &mut self,
        rows: &[NoteRow],
        rate: f32,
        goal: f32,
        keycount: u8,
    ) -> Result<Skillsets, CalcError> {
        let input = Input::new(rows, rate, keycount)?;
        if !(goal.is_finite() && goal > 0.0) {
            return Err(CalcError::Native);
        }
        let mut out = [0.0f32; 8];
        // SAFETY: as in `msd`.
        let status = unsafe {
            wl_ssr(
                self.raw.as_ptr(),
                input.notes.as_ptr(),
                input.times.as_ptr(),
                input.notes.len(),
                rate,
                goal,
                c_uint::from(keycount),
                out.as_mut_ptr(),
            )
        };
        finish(status, out)
    }
}

impl Drop for Calc {
    fn drop(&mut self) {
        // SAFETY: `raw` came from wl_calc_new and is freed exactly once.
        unsafe { wl_calc_free(self.raw.as_ptr()) }
    }
}

struct Input {
    notes: Vec<u32>,
    times: Vec<f32>,
}

impl Input {
    /// Rejects everything upstream would mishandle (UB casts, asserts, out-of-range indexing)
    /// so the native side only ever sees well-formed charts.
    fn new(rows: &[NoteRow], rate: f32, keycount: u8) -> Result<Self, CalcError> {
        if rows.is_empty() {
            return Err(CalcError::Empty);
        }
        if !KEYCOUNTS.contains(&keycount) {
            return Err(CalcError::UnsupportedKeycount(keycount));
        }
        let full_mask = (1u32 << keycount) - 1;
        let mut previous: Option<f32> = None;
        for (index, row) in rows.iter().enumerate() {
            let t = row.time_s;
            let ordered = match previous {
                None => t >= 0.0,
                Some(p) => t > p,
            };
            if !(t.is_finite() && ordered) {
                return Err(CalcError::NotIncreasing { index });
            }
            if row.notes == 0 || row.notes > full_mask {
                return Err(CalcError::MaskOutOfRange { index });
            }
            previous = Some(t);
        }
        if !(rate.is_finite() && rate > 0.0) {
            return Err(CalcError::Native);
        }
        let last = previous.unwrap_or(0.0);
        let scaled = last / rate;
        if !(scaled.is_finite() && scaled < MAX_SCALED_TIME_S) {
            return Err(CalcError::Native);
        }
        Ok(Self {
            notes: rows.iter().map(|r| r.notes).collect(),
            times: rows.iter().map(|r| r.time_s).collect(),
        })
    }
}

fn finish(status: c_int, out: [f32; 8]) -> Result<Skillsets, CalcError> {
    if status == STATUS_OK {
        Ok(Skillsets(out))
    } else {
        Err(CalcError::Native)
    }
}

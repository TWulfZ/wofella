//! Signalsmith Stretch behind a C ABI (ADR 0025).
//!
//! Output is platform floats: compare it by tolerance (onsets, lengths), never by hash.

// ADR 0025: the second workspace crate allowed `unsafe`, confined to the FFI calls below.
#![allow(unsafe_code)]

use std::ffi::{c_float, c_int, c_void};
use std::fmt;
use std::ops::RangeInclusive;
use std::ptr::NonNull;

pub const CHANNELS: RangeInclusive<u16> = 1..=8;
pub const SAMPLE_RATES: RangeInclusive<u32> = 8_000..=192_000;
/// Upstream sounds best between 0.75x and 1.5x and randomises phases past a 2x stretch.
pub const RATES: RangeInclusive<f32> = 0.5..=2.0;

/// Upstream counts samples in `int`; output can be twice the input at 0.5x.
const MAX_FRAMES: usize = (i32::MAX / 4) as usize;

const STATUS_OK: c_int = 0;
const PRESET_DEFAULT: c_int = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StretchError {
    Channels(u16),
    SampleRate(u32),
    Rate,
    Ragged { samples: usize, channels: u16 },
    NonFinite { index: usize },
    TooLong { frames: usize },
    Native,
}

impl fmt::Display for StretchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channels(c) => write!(f, "{c} channels; supported are {CHANNELS:?}"),
            Self::SampleRate(r) => write!(f, "{r} Hz; supported are {SAMPLE_RATES:?}"),
            Self::Rate => write!(f, "rate is not finite or outside {RATES:?}"),
            Self::Ragged { samples, channels } => write!(
                f,
                "{samples} interleaved samples is not a whole number of {channels}-channel frames"
            ),
            Self::NonFinite { index } => write!(f, "sample {index} is not finite"),
            Self::TooLong { frames } => write!(f, "{frames} frames exceed {MAX_FRAMES}"),
            Self::Native => f.write_str("Signalsmith Stretch failed"),
        }
    }
}

impl std::error::Error for StretchError {}

unsafe extern "C" {
    fn wl_stretch_new(channels: c_int, sample_rate: c_float, preset: c_int) -> *mut c_void;
    fn wl_stretch_free(handle: *mut c_void);
    fn wl_stretch_process(
        handle: *mut c_void,
        input: *const c_float,
        in_frames: usize,
        output: *mut c_float,
        out_frames: usize,
    ) -> c_int;
}

/// One native stretcher. It mutates scratch buffers on every call, so use one per thread.
///
/// ```compile_fail
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<wolluf_signalsmith::Stretcher>();
/// ```
#[derive(Debug)]
pub struct Stretcher {
    // NonNull keeps Stretcher !Sync: the native object mutates itself on every call.
    raw: NonNull<c_void>,
    channels: u16,
}

// SAFETY: the native object owns all its state and has no thread affinity; only shared
// access from two threads at once would race, and Stretcher is not Sync.
unsafe impl Send for Stretcher {}

impl Stretcher {
    pub fn new(channels: u16, sample_rate: u32) -> Result<Self, StretchError> {
        if !CHANNELS.contains(&channels) {
            return Err(StretchError::Channels(channels));
        }
        if !SAMPLE_RATES.contains(&sample_rate) {
            return Err(StretchError::SampleRate(sample_rate));
        }
        // SAFETY: plain values; a null return (allocation or configure failure) is handled below.
        let raw = unsafe {
            wl_stretch_new(
                c_int::from(channels),
                sample_rate as c_float,
                PRESET_DEFAULT,
            )
        };
        NonNull::new(raw)
            .map(|raw| Self { raw, channels })
            .ok_or(StretchError::Native)
    }

    /// Plays `input` (interleaved) `rate` times faster with the pitch kept: the output has
    /// `round(frames / rate)` frames and stays aligned to the input (no pre-roll).
    pub fn stretch(&mut self, input: &[f32], rate: f32) -> Result<Vec<f32>, StretchError> {
        if !(rate.is_finite() && RATES.contains(&rate)) {
            return Err(StretchError::Rate);
        }
        let channels = usize::from(self.channels);
        if !input.len().is_multiple_of(channels) {
            return Err(StretchError::Ragged {
                samples: input.len(),
                channels: self.channels,
            });
        }
        if let Some(index) = input.iter().position(|s| !s.is_finite()) {
            return Err(StretchError::NonFinite { index });
        }
        let in_frames = input.len() / channels;
        if in_frames > MAX_FRAMES {
            return Err(StretchError::TooLong { frames: in_frames });
        }
        let out_frames = (in_frames as f64 / f64::from(rate)).round() as usize;
        if in_frames == 0 || out_frames == 0 {
            return Ok(Vec::new());
        }
        let mut output = vec![0.0f32; out_frames * channels];
        // SAFETY: `raw` is live and exclusively borrowed; `input` holds `in_frames * channels`
        // samples and `output` holds `out_frames * channels`, the channel count the handle
        // was configured with.
        let status = unsafe {
            wl_stretch_process(
                self.raw.as_ptr(),
                input.as_ptr(),
                in_frames,
                output.as_mut_ptr(),
                out_frames,
            )
        };
        if status == STATUS_OK {
            Ok(output)
        } else {
            Err(StretchError::Native)
        }
    }
}

impl Drop for Stretcher {
    fn drop(&mut self) {
        // SAFETY: `raw` came from wl_stretch_new and is freed exactly once.
        unsafe { wl_stretch_free(self.raw.as_ptr()) }
    }
}

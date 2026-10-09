#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wolluf_signalsmith::{StretchError, Stretcher};

const SR: u32 = 44_100;
const CHANNELS: u16 = 2;
const CLICK_PERIOD_S: f64 = 0.5;
// Offset from 0 so the first click is not clipped by the pre-roll.
const FIRST_CLICK_S: f64 = 0.25;
const TRACK_S: f64 = 30.0;

/// Interleaved stereo: left and right clicks differ in amplitude so a channel swap shows up.
fn click_track() -> Vec<f32> {
    let frames = (TRACK_S * f64::from(SR)) as usize;
    let mut out = vec![0.0f32; frames * usize::from(CHANNELS)];
    let mut t = FIRST_CLICK_S;
    while t < TRACK_S - 0.1 {
        let frame = (t * f64::from(SR)).round() as usize;
        out[frame * 2] = 0.9;
        out[frame * 2 + 1] = -0.6;
        t += CLICK_PERIOD_S;
    }
    out
}

/// Frame of the largest |sample| of `channel` within ±`radius` frames of `center`.
fn peak_frame(samples: &[f32], channel: usize, center: usize, radius: usize) -> usize {
    let frames = samples.len() / 2;
    let lo = center.saturating_sub(radius);
    let hi = (center + radius).min(frames - 1);
    (lo..=hi)
        .max_by(|&a, &b| {
            samples[a * 2 + channel]
                .abs()
                .total_cmp(&samples[b * 2 + channel].abs())
        })
        .unwrap()
}

fn assert_onsets_and_length(rate: f32) {
    let input = click_track();
    let mut stretcher = Stretcher::new(CHANNELS, SR).unwrap();
    let output = stretcher.stretch(&input, rate).unwrap();

    assert_eq!(output.len() % 2, 0);
    let in_s = (input.len() / 2) as f64 / f64::from(SR);
    let out_s = (output.len() / 2) as f64 / f64::from(SR);
    let expected_s = in_s / f64::from(rate);
    assert!(
        (out_s - expected_s).abs() <= 0.010,
        "rate {rate}: length {out_s:.4} s, expected {expected_s:.4} s"
    );

    let radius = (0.05 * f64::from(SR)) as usize;
    let mut worst_ms = 0.0f64;
    let mut t = FIRST_CLICK_S;
    while t < TRACK_S - 0.1 {
        let expected = t / f64::from(rate);
        let center = (expected * f64::from(SR)).round() as usize;
        for channel in 0..2 {
            let found = peak_frame(&output, channel, center, radius);
            assert!(
                output[found * 2 + channel].abs() > 0.1,
                "rate {rate}: click at {t:.2} s (ch {channel}) vanished"
            );
            let drift_ms = (found as f64 / f64::from(SR) - expected).abs() * 1000.0;
            worst_ms = worst_ms.max(drift_ms);
            assert!(
                drift_ms <= 2.0,
                "rate {rate}: click at {t:.2} s (ch {channel}) found {drift_ms:.3} ms off"
            );
        }
        t += CLICK_PERIOD_S;
    }
    eprintln!("rate {rate}: worst onset drift {worst_ms:.3} ms");
}

#[test]
fn click_track_onsets_hold_at_0_70() {
    assert_onsets_and_length(0.70);
}

#[test]
fn click_track_onsets_hold_at_0_85() {
    assert_onsets_and_length(0.85);
}

#[test]
fn click_track_onsets_hold_at_1_15() {
    assert_onsets_and_length(1.15);
}

#[test]
fn click_track_onsets_hold_at_1_50() {
    assert_onsets_and_length(1.50);
}

#[test]
fn channels_are_not_swapped() {
    let input = click_track();
    let output = Stretcher::new(CHANNELS, SR)
        .unwrap()
        .stretch(&input, 1.2)
        .unwrap();
    let center = (FIRST_CLICK_S / 1.2 * f64::from(SR)).round() as usize;
    let left = peak_frame(&output, 0, center, 2000);
    let right = peak_frame(&output, 1, center, 2000);
    assert!(output[left * 2] > 0.0, "left click keeps its sign");
    assert!(output[right * 2 + 1] < 0.0, "right click keeps its sign");
}

#[test]
fn repeated_calls_are_identical() {
    let input = click_track();
    let mut stretcher = Stretcher::new(CHANNELS, SR).unwrap();
    let first = stretcher.stretch(&input, 0.75).unwrap();
    let second = stretcher.stretch(&input, 0.75).unwrap();
    let fresh = Stretcher::new(CHANNELS, SR)
        .unwrap()
        .stretch(&input, 0.75)
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first, fresh);
}

#[test]
fn rate_one_keeps_length_exactly() {
    let input = click_track();
    let output = Stretcher::new(CHANNELS, SR)
        .unwrap()
        .stretch(&input, 1.0)
        .unwrap();
    assert_eq!(output.len(), input.len());
}

#[test]
fn rejects_bad_configuration() {
    assert_eq!(
        Stretcher::new(0, SR).unwrap_err(),
        StretchError::Channels(0)
    );
    assert_eq!(
        Stretcher::new(9, SR).unwrap_err(),
        StretchError::Channels(9)
    );
    assert_eq!(
        Stretcher::new(2, 7_999).unwrap_err(),
        StretchError::SampleRate(7_999)
    );
    assert_eq!(
        Stretcher::new(2, 192_001).unwrap_err(),
        StretchError::SampleRate(192_001)
    );
    assert!(Stretcher::new(1, 8_000).is_ok());
    assert!(Stretcher::new(8, 192_000).is_ok());
}

#[test]
fn rejects_bad_input() {
    let mut stretcher = Stretcher::new(2, SR).unwrap();
    let ok = vec![0.0f32; 2000];
    for rate in [0.49, 2.01, 0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            stretcher.stretch(&ok, rate).unwrap_err(),
            StretchError::Rate,
            "rate {rate}"
        );
    }
    assert!(stretcher.stretch(&ok, 0.5).is_ok());
    assert!(stretcher.stretch(&ok, 2.0).is_ok());
    assert_eq!(
        stretcher.stretch(&[0.0; 3], 1.0).unwrap_err(),
        StretchError::Ragged {
            samples: 3,
            channels: 2
        }
    );
    let mut nan = ok.clone();
    nan[7] = f32::NAN;
    assert_eq!(
        stretcher.stretch(&nan, 1.0).unwrap_err(),
        StretchError::NonFinite { index: 7 }
    );
    let mut inf = ok;
    inf[0] = f32::NEG_INFINITY;
    assert_eq!(
        stretcher.stretch(&inf, 1.0).unwrap_err(),
        StretchError::NonFinite { index: 0 }
    );
}

#[test]
fn empty_and_short_inputs_do_not_crash() {
    let mut stretcher = Stretcher::new(2, SR).unwrap();
    assert!(stretcher.stretch(&[], 1.3).unwrap().is_empty());
    for frames in [1usize, 2, 10, 100, 1000, 6000, 10_000] {
        for rate in [0.5f32, 0.7, 1.0, 1.5, 2.0] {
            let input: Vec<f32> = (0..frames * 2)
                .map(|i| if i % 97 == 0 { 0.5 } else { 0.0 })
                .collect();
            let out = stretcher.stretch(&input, rate).unwrap();
            let expected = (frames as f64 / f64::from(rate)).round() as usize;
            assert_eq!(out.len(), expected * 2, "{frames} frames at {rate}");
            assert!(out.iter().all(|s| s.is_finite()));
        }
    }
}

#[test]
fn mono_works() {
    let input: Vec<f32> = click_track().into_iter().step_by(2).collect();
    let out = Stretcher::new(1, SR).unwrap().stretch(&input, 1.5).unwrap();
    let expected = (input.len() as f64 / 1.5).round() as usize;
    assert_eq!(out.len(), expected);
}

const TONE_HZ: f64 = 440.0;

/// Taylor series after range reduction: the workspace bans platform libm (D3), even in tests.
fn sine(phase: f64) -> f64 {
    let tau = 2.0 * std::f64::consts::PI;
    let x = phase - tau * (phase / tau).round();
    let (mut term, mut sum) = (x, x);
    for n in 1..12 {
        term *= -x * x / f64::from((2 * n) * (2 * n + 1));
        sum += term;
    }
    sum
}

fn mono_tone(seconds: f64) -> Vec<f32> {
    let frames = (seconds * f64::from(SR)) as usize;
    (0..frames)
        .map(|i| {
            let t = i as f64 / f64::from(SR);
            (0.5 * sine(2.0 * std::f64::consts::PI * TONE_HZ * t)) as f32
        })
        .collect()
}

/// Frequency from the rising zero crossings of the middle half, interpolated between samples.
fn zero_crossing_hz(samples: &[f32]) -> f64 {
    let (lo, hi) = (samples.len() / 4, samples.len() * 3 / 4);
    let crossings: Vec<f64> = (lo..hi)
        .filter(|&i| samples[i] <= 0.0 && samples[i + 1] > 0.0)
        .map(|i| {
            let (a, b) = (f64::from(samples[i]), f64::from(samples[i + 1]));
            i as f64 + a / (a - b)
        })
        .collect();
    assert!(
        crossings.len() > 10,
        "too few crossings: {}",
        crossings.len()
    );
    let span = crossings[crossings.len() - 1] - crossings[0];
    (crossings.len() - 1) as f64 * f64::from(SR) / span
}

#[test]
fn transpose_equal_to_rate_raises_the_pitch_like_nightcore() {
    let input = mono_tone(3.0);
    let out = Stretcher::new(1, SR)
        .unwrap()
        .stretch_transposed(&input, 1.5, 1.5)
        .unwrap();
    assert_eq!(out.len(), (input.len() as f64 / 1.5).round() as usize);
    let hz = zero_crossing_hz(&out);
    let expected = TONE_HZ * 1.5;
    assert!(
        (hz - expected).abs() <= expected * 0.02,
        "{hz:.2} Hz, expected {expected:.2} Hz"
    );
}

#[test]
fn transpose_one_keeps_the_pitch() {
    let input = mono_tone(3.0);
    let mut stretcher = Stretcher::new(1, SR).unwrap();
    let out = stretcher.stretch_transposed(&input, 1.5, 1.0).unwrap();
    assert_eq!(out.len(), (input.len() as f64 / 1.5).round() as usize);
    let hz = zero_crossing_hz(&out);
    assert!(
        (hz - TONE_HZ).abs() <= TONE_HZ * 0.02,
        "{hz:.2} Hz, expected {TONE_HZ:.2} Hz"
    );
    assert_eq!(out, stretcher.stretch(&input, 1.5).unwrap());
}

#[test]
fn transposed_renders_are_repeatable() {
    let input = click_track();
    let mut stretcher = Stretcher::new(CHANNELS, SR).unwrap();
    let first = stretcher.stretch_transposed(&input, 1.3, 1.3).unwrap();
    let pitched_down = stretcher.stretch_transposed(&input, 0.8, 0.8).unwrap();
    let second = stretcher.stretch_transposed(&input, 1.3, 1.3).unwrap();
    assert_eq!(first, second, "a transpose never leaks into the next call");
    assert_ne!(first.len(), pitched_down.len());
    assert_ne!(first, stretcher.stretch(&input, 1.3).unwrap());
}

#[test]
fn rejects_bad_transpose_factors() {
    let mut stretcher = Stretcher::new(2, SR).unwrap();
    let ok = vec![0.0f32; 2000];
    for factor in [0.49, 2.01, 0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            stretcher.stretch_transposed(&ok, 1.0, factor).unwrap_err(),
            StretchError::Transpose,
            "factor {factor}"
        );
    }
    assert!(stretcher.stretch_transposed(&ok, 1.0, 0.5).is_ok());
    assert!(stretcher.stretch_transposed(&ok, 1.0, 2.0).is_ok());
}

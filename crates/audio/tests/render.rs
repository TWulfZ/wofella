#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wolluf_audio::{AudioError, AudioParams, Pcm, decode, render_rate};

const CLICK_PERIOD_S: f64 = 0.5;
const FIRST_CLICK_S: f64 = 0.25;

/// 16-bit PCM WAV built in memory, so no audio file is committed.
fn wav(
    channels: u16,
    sample_rate: u32,
    frames: usize,
    sample: impl Fn(usize, u16) -> f32,
) -> Vec<u8> {
    let data_len = frames * usize::from(channels) * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32::try_from(36 + data_len).unwrap().to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&u32::try_from(data_len).unwrap().to_le_bytes());
    for f in 0..frames {
        for c in 0..channels {
            let v = (sample(f, c).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

fn is_click(frame: usize, sample_rate: u32) -> bool {
    let t = frame as f64 / f64::from(sample_rate);
    let k = ((t - FIRST_CLICK_S) / CLICK_PERIOD_S).round();
    k >= 0.0
        && frame == ((FIRST_CLICK_S + k * CLICK_PERIOD_S) * f64::from(sample_rate)).round() as usize
}

fn click_wav(channels: u16, sample_rate: u32, seconds: f64) -> Vec<u8> {
    let frames = (seconds * f64::from(sample_rate)) as usize;
    wav(channels, sample_rate, frames, |f, c| {
        if is_click(f, sample_rate) {
            if c == 0 { 0.9 } else { -0.6 }
        } else {
            0.0
        }
    })
}

/// Deterministic pseudo-noise (LCG), loud enough for the Vorbis quality to matter.
fn noise_wav(seconds: f64) -> Vec<u8> {
    let frames = (seconds * 44_100.0) as usize;
    let state = std::cell::Cell::new(0x1234_5678u32);
    wav(2, 44_100, frames, |_, _| {
        let s = state
            .get()
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        state.set(s);
        (f64::from(s >> 8) / f64::from(1u32 << 24) - 0.5) as f32 * 0.6
    })
}

fn seconds(pcm: &Pcm) -> f64 {
    (pcm.samples.len() / usize::from(pcm.channels)) as f64 / f64::from(pcm.sample_rate)
}

fn peak_frame(pcm: &Pcm, channel: usize, center: usize, radius: usize) -> usize {
    let ch = usize::from(pcm.channels);
    let frames = pcm.samples.len() / ch;
    let lo = center.saturating_sub(radius);
    let hi = (center + radius).min(frames - 1);
    (lo..=hi)
        .max_by(|&a, &b| {
            pcm.samples[a * ch + channel]
                .abs()
                .total_cmp(&pcm.samples[b * ch + channel].abs())
        })
        .unwrap()
}

fn assert_wav_round_trip(rate_milli: u16) {
    let source_s = 10.0;
    let input = click_wav(2, 44_100, source_s);
    let ogg = render_rate(&input, "wav", rate_milli, &AudioParams::default()).unwrap();
    assert_eq!(&ogg[..4], b"OggS");

    let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    assert_eq!(out.channels, 2);
    assert_eq!(out.sample_rate, 44_100);
    let r = f64::from(rate_milli) / 1000.0;
    let expected_s = source_s / r;
    assert!(
        (seconds(&out) - expected_s).abs() <= 0.010,
        "{rate_milli}: {:.4} s, expected {expected_s:.4} s",
        seconds(&out)
    );

    // Vorbis must not shift the audio either: the copy's .osu times assume t / r.
    let mut worst_ms = 0.0f64;
    let mut t = FIRST_CLICK_S;
    while t < source_s - 0.1 {
        let expected = t / r;
        let center = (expected * 44_100.0).round() as usize;
        for channel in 0..2 {
            let found = peak_frame(&out, channel, center, 2_205);
            let drift_ms = (found as f64 / 44_100.0 - expected).abs() * 1000.0;
            worst_ms = worst_ms.max(drift_ms);
            assert!(
                drift_ms <= 2.0,
                "{rate_milli}: click at {t:.2} s ch {channel} {drift_ms:.3} ms off"
            );
        }
        t += CLICK_PERIOD_S;
    }
    eprintln!("{rate_milli}: worst onset drift after Vorbis {worst_ms:.3} ms");
}

#[test]
fn wav_round_trip_0_70() {
    assert_wav_round_trip(700);
}

#[test]
fn wav_round_trip_0_85() {
    assert_wav_round_trip(850);
}

#[test]
fn wav_round_trip_1_15() {
    assert_wav_round_trip(1150);
}

#[test]
fn wav_round_trip_1_50() {
    assert_wav_round_trip(1500);
}

#[test]
fn mono_48k_is_preserved() {
    let input = click_wav(1, 48_000, 3.0);
    let ogg = render_rate(&input, "wav", 1200, &AudioParams::default()).unwrap();
    let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    assert_eq!(out.channels, 1);
    assert_eq!(out.sample_rate, 48_000);
    assert!((seconds(&out) - 3.0 / 1.2).abs() <= 0.010);
}

#[test]
fn decode_reads_the_wav_as_written() {
    let pcm = decode(&click_wav(2, 44_100, 2.0), "wav", &AudioParams::default()).unwrap();
    assert_eq!(pcm.channels, 2);
    assert_eq!(pcm.sample_rate, 44_100);
    assert_eq!(pcm.samples.len(), 2 * 88_200);
    let first = (FIRST_CLICK_S * 44_100.0) as usize;
    assert!((pcm.samples[first * 2] - 0.9).abs() < 1e-3);
    assert!((pcm.samples[first * 2 + 1] + 0.6).abs() < 1e-3);
}

#[test]
fn extension_hint_is_optional_and_tolerant() {
    let input = click_wav(2, 44_100, 1.0);
    for hint in ["wav", ".WAV", "", "mp3"] {
        let pcm = decode(&input, hint, &AudioParams::default()).unwrap();
        assert_eq!(pcm.samples.len(), 2 * 44_100, "hint {hint:?}");
    }
}

#[test]
fn quality_changes_the_bitrate() {
    let input = noise_wav(3.0);
    let low = render_rate(
        &input,
        "wav",
        1000,
        &AudioParams {
            vorbis_quality: 0.0,
            ..AudioParams::default()
        },
    )
    .unwrap();
    let high = render_rate(
        &input,
        "wav",
        1000,
        &AudioParams {
            vorbis_quality: 0.9,
            ..AudioParams::default()
        },
    )
    .unwrap();
    assert!(
        high.len() > low.len() * 3 / 2,
        "low {} B, high {} B",
        low.len(),
        high.len()
    );
}

#[test]
fn output_is_deterministic() {
    let input = click_wav(2, 44_100, 2.0);
    let a = render_rate(&input, "wav", 1300, &AudioParams::default()).unwrap();
    let b = render_rate(&input, "wav", 1300, &AudioParams::default()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn default_quality_is_half_and_mp3_is_gapless() {
    assert!((AudioParams::default().vorbis_quality - 0.5).abs() < f32::EPSILON);
    assert!(AudioParams::default().mp3_gapless);
}

#[test]
fn decode_reserves_the_declared_length_up_front() {
    let pcm = decode(&click_wav(2, 44_100, 2.0), "wav", &AudioParams::default()).unwrap();
    assert_eq!(pcm.samples.capacity(), pcm.samples.len());
}

#[test]
fn rejects_bad_arguments() {
    let input = click_wav(2, 44_100, 1.0);
    let p = AudioParams::default();
    assert!(matches!(
        render_rate(&input, "wav", 499, &p),
        Err(AudioError::Rate(499))
    ));
    assert!(matches!(
        render_rate(&input, "wav", 2001, &p),
        Err(AudioError::Rate(2001))
    ));
    assert!(render_rate(&input, "wav", 500, &p).is_ok());
    assert!(render_rate(&input, "wav", 2000, &p).is_ok());
    for q in [-0.21, 1.01, f32::NAN] {
        let bad = AudioParams {
            vorbis_quality: q,
            ..AudioParams::default()
        };
        assert!(
            matches!(
                render_rate(&input, "wav", 1000, &bad),
                Err(AudioError::Quality(_))
            ),
            "{q}"
        );
    }
}

#[test]
fn rejects_undecodable_input() {
    let p = AudioParams::default();
    assert!(matches!(
        render_rate(&[], "mp3", 1200, &p),
        Err(AudioError::Decode(_))
    ));
    assert!(matches!(
        render_rate(b"definitely not audio, just text bytes", "ogg", 1200, &p),
        Err(AudioError::Decode(_))
    ));
}

#[test]
fn rejects_audio_without_samples() {
    let empty = wav(2, 44_100, 0, |_, _| 0.0);
    assert!(matches!(
        render_rate(&empty, "wav", 1200, &AudioParams::default()),
        Err(AudioError::Empty)
    ));
}

#[test]
fn rejects_unsupported_layouts() {
    let low_rate = wav(1, 4_000, 4_000, |_, _| 0.0);
    assert!(matches!(
        render_rate(&low_rate, "wav", 1200, &AudioParams::default()),
        Err(AudioError::Stretch(_))
    ));
}

// MPEG-1 Layer III, 128 kbit/s, 44.1 kHz, joint-less stereo, no CRC: 417-byte frames.
const MP3_FRAME_LEN: usize = 417;
const MP3_FRAME_SAMPLES: usize = 1152;

/// Silent MP3 frames written by hand (all-zero side info and main data), so the test
/// needs neither an encoder nor a committed file. `corrupt` frames get big_values > 288.
fn silent_mp3(frames: usize, corrupt: &[usize]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frames * MP3_FRAME_LEN);
    for i in 0..frames {
        let mut frame = vec![0u8; MP3_FRAME_LEN];
        frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        if corrupt.contains(&i) {
            // Side info bits 32..41 are granule 0 / channel 0 big_values.
            frame[4 + 4] = 0xFF;
            frame[4 + 5] = 0x80;
        }
        out.extend_from_slice(&frame);
    }
    out
}

#[test]
fn mp3_input_is_rendered() {
    let mp3 = silent_mp3(200, &[]);
    let source = decode(&mp3, "mp3", &AudioParams::default()).unwrap();
    assert_eq!(source.channels, 2);
    assert_eq!(source.sample_rate, 44_100);
    assert!(source.samples.len() / 2 >= 190 * MP3_FRAME_SAMPLES);

    let ogg = render_rate(&mp3, "mp3", 1250, &AudioParams::default()).unwrap();
    let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    assert_eq!(out.channels, 2);
    assert_eq!(out.sample_rate, 44_100);
    assert!((seconds(&out) - seconds(&source) / 1.25).abs() <= 0.010);
}

#[test]
fn a_bad_mp3_frame_keeps_its_time_as_silence() {
    let clean = decode(&silent_mp3(200, &[]), "mp3", &AudioParams::default()).unwrap();
    let damaged = decode(&silent_mp3(200, &[50, 120]), "mp3", &AudioParams::default()).unwrap();
    assert_eq!(damaged.samples.len(), clean.samples.len());
}

#[test]
fn a_truncated_mp3_keeps_what_decoded() {
    let mut mp3 = silent_mp3(200, &[]);
    mp3.truncate(mp3.len() - MP3_FRAME_LEN / 2);
    let pcm = decode(&mp3, "mp3", &AudioParams::default()).unwrap();
    assert!(pcm.samples.len() / 2 >= 190 * MP3_FRAME_SAMPLES);
}

#[test]
fn a_truncated_wav_keeps_what_decoded() {
    let mut wav_bytes = click_wav(2, 44_100, 2.0);
    wav_bytes.truncate(wav_bytes.len() - 1001);
    let pcm = decode(&wav_bytes, "wav", &AudioParams::default()).unwrap();
    assert!(pcm.samples.len() / 2 >= 80_000);
}

// As a LAME tag stores them. Symphonia moves the 529-frame decoder delay from the padding to
// the delay, so the frames it trims still add up to delay + padding.
const LAME_ENC_DELAY: u32 = 576;
const LAME_ENC_PADDING: u32 = 1200;

/// `silent_mp3` behind an `Info` frame carrying a frame count and a LAME extension, the layout
/// LAME writes at the start of a CBR file.
fn lame_tagged_mp3(frames: usize) -> Vec<u8> {
    let mut tag = vec![0u8; MP3_FRAME_LEN];
    tag[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    // Header (4) + MPEG-1 stereo side info (32); the side info stays zero.
    let mut at = 36;
    let mut put = |bytes: &[u8]| {
        tag[at..at + bytes.len()].copy_from_slice(bytes);
        at += bytes.len();
    };
    put(b"Info");
    put(&1u32.to_be_bytes());
    put(&u32::try_from(frames).unwrap().to_be_bytes());
    put(b"LAME3.100");
    // Revision, lowpass, peak, radio and audiophile gain, flags, ABR bitrate: all zero.
    put(&[0; 1 + 1 + 4 + 2 + 2 + 1 + 1]);
    let trim = (LAME_ENC_DELAY << 12) | LAME_ENC_PADDING;
    put(&trim.to_be_bytes()[1..]);
    // Misc, mp3 gain, preset, music length, music CRC; a zero tag CRC means "not checked".
    put(&[0; 1 + 1 + 2 + 4 + 2 + 2]);
    [tag, silent_mp3(frames, &[])].concat()
}

fn frames_of(pcm: &Pcm) -> usize {
    pcm.samples.len() / usize::from(pcm.channels)
}

// Open question: whether stable's BASS trims the LAME delay the same way is checked in the
// pilot's Windows E2E; until then the default follows BASS's documented gapless handling.
#[test]
fn lame_delay_and_padding_follow_mp3_gapless() {
    let frames = 200;
    let mp3 = lame_tagged_mp3(frames);
    let trim = (LAME_ENC_DELAY + LAME_ENC_PADDING) as usize;

    let gapless = decode(&mp3, "mp3", &AudioParams::default()).unwrap();
    assert_eq!(frames_of(&gapless), frames * MP3_FRAME_SAMPLES - trim);

    let raw = AudioParams {
        mp3_gapless: false,
        ..AudioParams::default()
    };
    let untrimmed = decode(&mp3, "mp3", &raw).unwrap();
    assert_eq!(frames_of(&untrimmed), frames * MP3_FRAME_SAMPLES);

    // An Ogg's granule-based trimming is part of the format, not an MP3 setting.
    let ogg = render_rate(
        &click_wav(2, 44_100, 1.0),
        "wav",
        1250,
        &AudioParams::default(),
    )
    .unwrap();
    assert_eq!(
        decode(&ogg, "ogg", &raw).unwrap().samples.len(),
        decode(&ogg, "ogg", &AudioParams::default())
            .unwrap()
            .samples
            .len()
    );
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

fn tone_wav(seconds: f64) -> Vec<u8> {
    let frames = (seconds * 44_100.0) as usize;
    wav(2, 44_100, frames, |f, _| {
        (0.5 * sine(2.0 * std::f64::consts::PI * TONE_HZ * f as f64 / 44_100.0)) as f32
    })
}

/// Frequency of channel 0 from the rising zero crossings of the middle half.
fn zero_crossing_hz(pcm: &Pcm) -> f64 {
    let ch = usize::from(pcm.channels);
    let left: Vec<f64> = pcm
        .samples
        .iter()
        .step_by(ch)
        .map(|&s| f64::from(s))
        .collect();
    let (lo, hi) = (left.len() / 4, left.len() * 3 / 4);
    let crossings: Vec<f64> = (lo..hi)
        .filter(|&i| left[i] <= 0.0 && left[i + 1] > 0.0)
        .map(|i| i as f64 + left[i] / (left[i] - left[i + 1]))
        .collect();
    assert!(
        crossings.len() > 10,
        "too few crossings: {}",
        crossings.len()
    );
    let span = crossings[crossings.len() - 1] - crossings[0];
    (crossings.len() - 1) as f64 * f64::from(pcm.sample_rate) / span
}

fn nightcore() -> AudioParams {
    AudioParams {
        pitch_follows_rate: true,
        ..AudioParams::default()
    }
}

#[test]
fn pitch_is_kept_by_default() {
    assert!(!AudioParams::default().pitch_follows_rate);
    let ogg = render_rate(&tone_wav(3.0), "wav", 1500, &AudioParams::default()).unwrap();
    let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    let hz = zero_crossing_hz(&out);
    assert!((hz - TONE_HZ).abs() <= TONE_HZ * 0.02, "{hz:.2} Hz");
}

#[test]
fn nightcore_round_trip_raises_the_pitch_by_the_rate() {
    for rate_milli in [1500u16, 800] {
        let ogg = render_rate(&tone_wav(3.0), "wav", rate_milli, &nightcore()).unwrap();
        let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
        let r = f64::from(rate_milli) / 1000.0;
        assert!(
            (seconds(&out) - 3.0 / r).abs() <= 0.010,
            "{rate_milli}: {:.4} s",
            seconds(&out)
        );
        let hz = zero_crossing_hz(&out);
        let expected = TONE_HZ * r;
        assert!(
            (hz - expected).abs() <= expected * 0.02,
            "{rate_milli}: {hz:.2} Hz, expected {expected:.2} Hz"
        );
    }
}

#[test]
fn nightcore_keeps_the_onsets_on_the_copy_times() {
    let source_s = 10.0;
    let ogg = render_rate(&click_wav(2, 44_100, source_s), "wav", 1250, &nightcore()).unwrap();
    let out = decode(&ogg, "ogg", &AudioParams::default()).unwrap();
    let mut t = FIRST_CLICK_S;
    while t < source_s - 0.1 {
        let expected = t / 1.25;
        let center = (expected * 44_100.0).round() as usize;
        let found = peak_frame(&out, 0, center, 2_205);
        let drift_ms = (found as f64 / 44_100.0 - expected).abs() * 1000.0;
        assert!(drift_ms <= 2.0, "click at {t:.2} s {drift_ms:.3} ms off");
        t += CLICK_PERIOD_S;
    }
}

//! Audio decode, time-stretch and Ogg Vorbis encode for rate copies (ADR 0025).
//!
//! Bytes in, bytes out: the app reads the source file and writes the copy.

use std::io::Cursor;
use std::num::{NonZeroU8, NonZeroU32};
use std::ops::RangeInclusive;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::audio::well_known::CODEC_ID_MP3;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder, VorbisError};
use wolluf_signalsmith::{StretchError, Stretcher};

/// The stretcher's supported range (0.5x..=2.0x), in thousandths.
pub const RATES_MILLI: RangeInclusive<u16> = 500..=2000;
/// libvorbis' quality scale.
pub const VORBIS_QUALITY: RangeInclusive<f32> = -0.2..=1.0;

// A fixed serial keeps the output reproducible; one logical stream per file needs no uniqueness.
const OGG_STREAM_SERIAL: i32 = 0x776f_6c66;
// libvorbis calls 1024 frames "a reasonable choice"; much larger blocks slow it down.
const ENCODE_BLOCK_FRAMES: usize = 4096;
// The up-front reservation trusts a header's frame count only this far, so a corrupt count
// cannot request a huge buffer; longer audio still decodes, it just grows the buffer.
const MAX_RESERVED_SECONDS: u64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioParams {
    pub vorbis_quality: f32,
    /// Trims the encoder delay and padding a LAME/Xing tag records, as BASS does by default.
    /// `false` keeps the decoder's whole output, leading delay included, which is what a player
    /// that ignores the tag hears.
    pub mp3_gapless: bool,
    /// NC: the pitch rises and falls with the rate. `false` keeps it, as DT does (ADR 0025).
    pub pitch_follows_rate: bool,
}

impl Default for AudioParams {
    fn default() -> Self {
        // ≈ 80 kbit/s for 44.1 kHz stereo (libvorbis), close to typical osu! mp3s.
        Self {
            vorbis_quality: 0.5,
            // Open question: whether stable's BASS trims too is checked in the pilot's Windows E2E.
            mp3_gapless: true,
            pitch_follows_rate: false,
        }
    }
}

/// Interleaved f32 samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("rate {0}/1000 is outside {RATES_MILLI:?}")]
    Rate(u16),
    #[error("Vorbis quality {0} is outside {VORBIS_QUALITY:?}")]
    Quality(f32),
    #[error("no decodable audio track")]
    NoTrack,
    #[error("the audio has no samples")]
    Empty,
    #[error("the sample rate or channel count changes mid-stream")]
    SpecChanged,
    #[error("cannot decode the audio: {0}")]
    Decode(#[from] SymphoniaError),
    #[error("cannot time-stretch the audio: {0}")]
    Stretch(#[from] StretchError),
    #[error("cannot encode Ogg Vorbis: {0}")]
    Encode(#[from] VorbisError),
}

/// Decodes `audio`, plays it `rate_milli / 1000` times faster (pitch kept unless
/// `params.pitch_follows_rate`), and encodes the result as Ogg Vorbis with the source's sample
/// rate and channels.
pub fn render_rate(
    audio: &[u8],
    ext_hint: &str,
    rate_milli: u16,
    params: &AudioParams,
) -> Result<Vec<u8>, AudioError> {
    if !RATES_MILLI.contains(&rate_milli) {
        return Err(AudioError::Rate(rate_milli));
    }
    if !(params.vorbis_quality.is_finite() && VORBIS_QUALITY.contains(&params.vorbis_quality)) {
        return Err(AudioError::Quality(params.vorbis_quality));
    }
    let Pcm {
        samples,
        channels,
        sample_rate,
    } = decode(audio, ext_hint, params)?;
    let rate = f32::from(rate_milli) / 1000.0;
    let transpose = if params.pitch_follows_rate { rate } else { 1.0 };
    let stretched =
        Stretcher::new(channels, sample_rate)?.stretch_transposed(&samples, rate, transpose)?;
    // Peak memory then holds one full-length PCM buffer during encoding, not two.
    drop(samples);
    encode_vorbis(
        &Pcm {
            samples: stretched,
            channels,
            sample_rate,
        },
        params.vorbis_quality,
    )
}

/// Decodes the default audio track to interleaved f32. Encoder delay and padding are trimmed
/// where the file records them: always for Ogg, for MP3 only with `params.mp3_gapless`.
pub fn decode(audio: &[u8], ext_hint: &str, params: &AudioParams) -> Result<Pcm, AudioError> {
    let mut hint = Hint::new();
    let ext = ext_hint.trim_start_matches('.').to_ascii_lowercase();
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }
    let mss = MediaSourceStream::new(Box::new(Cursor::new(audio)), Default::default());
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        mss,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioError::NoTrack)?;
    let track_id = track.id;
    let time_base = track.time_base;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(AudioError::NoTrack)?
        .clone();
    let gapless = params.mp3_gapless || codec_params.codec != CODEC_ID_MP3;
    // `num_frames` excludes the delay and padding the decoder trims when gapless.
    let expected_frames = track.num_frames.map(|n| {
        if gapless {
            n
        } else {
            n + u64::from(track.delay.unwrap_or(0)) + u64::from(track.padding.unwrap_or(0))
        }
    });
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(
        &codec_params,
        &AudioDecoderOptions::default().gapless(gapless),
    )?;

    let declared = match (codec_params.channels.as_ref(), codec_params.sample_rate) {
        (Some(c), Some(r)) => u16::try_from(c.count()).ok().map(|c| (c, r)),
        _ => None,
    };
    let mut spec: Option<(u16, u32)> = None;
    let mut samples: Vec<f32> = Vec::new();
    let reserve = expected_frames
        .zip(declared)
        .and_then(|(frames, (channels, rate))| {
            let frames = frames.min(MAX_RESERVED_SECONDS * u64::from(rate));
            usize::try_from(frames)
                .ok()?
                .checked_mul(usize::from(channels))
        });
    if let Some(len) = reserve {
        // Only a hint: if the allocator refuses, the loop below grows the buffer as before.
        let _ = samples.try_reserve_exact(len);
    }
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // A cut-off WAV ends in this error; the audio before the cut is still usable.
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof && !samples.is_empty() =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let this = (
                    u16::try_from(decoded.spec().channels().count())
                        .map_err(|_| AudioError::SpecChanged)?,
                    decoded.spec().rate(),
                );
                if *spec.get_or_insert(this) != this {
                    return Err(AudioError::SpecChanged);
                }
                let start = samples.len();
                samples.resize(start + decoded.samples_interleaved(), 0.0);
                decoded.copy_to_slice_interleaved(&mut samples[start..]);
            }
            // Dropping a bad packet would pull every later note early by its length, so
            // its time is kept as silence when the packet duration is in frames.
            Err(SymphoniaError::DecodeError(msg)) => {
                let (channels, rate) = spec.or(declared).ok_or(SymphoniaError::DecodeError(msg))?;
                let in_frames =
                    time_base.is_some_and(|tb| tb.numer.get() == 1 && tb.denom.get() == rate);
                // Real packets are at most a few thousand frames; a corrupt duration must
                // not turn into a huge allocation.
                let frames = usize::try_from(packet.dur.get())
                    .ok()
                    .filter(|&f| in_frames && f <= rate as usize);
                let Some(frames) = frames else {
                    return Err(SymphoniaError::DecodeError(msg).into());
                };
                spec.get_or_insert((channels, rate));
                samples.resize(samples.len() + frames * usize::from(channels), 0.0);
            }
            Err(e) => return Err(e.into()),
        }
    }
    let (channels, sample_rate) = spec.ok_or(AudioError::Empty)?;
    if samples.is_empty() {
        return Err(AudioError::Empty);
    }
    Ok(Pcm {
        samples,
        channels,
        sample_rate,
    })
}

fn encode_vorbis(pcm: &Pcm, quality: f32) -> Result<Vec<u8>, AudioError> {
    let channels = NonZeroU8::new(u8::try_from(pcm.channels).unwrap_or(0))
        .ok_or(StretchError::Channels(pcm.channels))?;
    let sample_rate =
        NonZeroU32::new(pcm.sample_rate).ok_or(StretchError::SampleRate(pcm.sample_rate))?;
    let mut encoder =
        VorbisEncoderBuilder::new_with_serial(sample_rate, channels, Vec::new(), OGG_STREAM_SERIAL)
            .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
                target_quality: quality,
            })
            .build()?;
    let ch = usize::from(pcm.channels);
    let mut planes = vec![Vec::with_capacity(ENCODE_BLOCK_FRAMES); ch];
    for block in pcm.samples.chunks(ENCODE_BLOCK_FRAMES * ch) {
        for (c, plane) in planes.iter_mut().enumerate() {
            plane.clear();
            plane.extend(block.iter().skip(c).step_by(ch));
        }
        encoder.encode_audio_block(&planes)?;
    }
    Ok(encoder.finish()?)
}

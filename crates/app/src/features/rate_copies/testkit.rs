//! Synthetic charts and audio for the rate-copy tests: nothing real is committed.

/// A v14 osu!mania file at 120 BPM. `taps` are `(column, ms)`, `holds` `(column, head, tail)`;
/// `events` go into `[Events]` as they are.
pub(super) struct OsuChart<'a> {
    pub(super) keys: u8,
    pub(super) title: &'a str,
    pub(super) audio: &'a str,
    pub(super) tags: &'a str,
    pub(super) taps: Vec<(u8, i32)>,
    pub(super) holds: Vec<(u8, i32, i32)>,
    pub(super) events: &'a [&'a str],
    /// `[Variables]` lines; the section is left out when empty.
    pub(super) variables: &'a [&'a str],
    /// Appended to every tap's hit-sample field: a file name there makes the chart keysounded.
    pub(super) tap_sample_file: &'a str,
}

impl<'a> OsuChart<'a> {
    /// Eight 7K taps, 250 ms apart from 1 s.
    pub(super) fn k7(title: &'a str) -> Self {
        Self {
            keys: 7,
            title,
            audio: "audio.wav",
            tags: "",
            taps: (0..8)
                .map(|i| (i % 7, 1_000 + i32::from(i) * 250))
                .collect(),
            holds: Vec::new(),
            events: &[],
            variables: &[],
            tap_sample_file: "",
        }
    }

    /// Mostly holds: far above the difficulty stage's LN cut-off.
    pub(super) fn ln_heavy(title: &'a str) -> Self {
        Self {
            taps: vec![(0, 9_000)],
            holds: (0..16)
                .map(|i| {
                    let head = 1_000 + i * 300;
                    (u8::try_from(i % 7).unwrap(), head, head + 250)
                })
                .collect(),
            ..Self::k7(title)
        }
    }

    pub(super) fn bytes(&self) -> Vec<u8> {
        let x = |col: u8| (2 * u32::from(col) + 1) * 256 / u32::from(self.keys);
        let mut text = format!(
            "osu file format v14\r\n\r\n[General]\r\nAudioFilename: {}\r\nAudioLeadIn: 0\r\n\
             PreviewTime: 2000\r\nMode: 3\r\n\r\n[Metadata]\r\nTitle:{}\r\nArtist:wolluf\r\n\
             Creator:wolluf\r\nVersion:Normal\r\nTags:{}\r\nBeatmapID:5\r\nBeatmapSetID:100\r\n\r\n\
             [Difficulty]\r\nHPDrainRate:8\r\nCircleSize:{}\r\nOverallDifficulty:8\r\n\r\n\
             {}[Events]\r\n{}\r\n\r\n[TimingPoints]\r\n0,500,4,1,0,100,1,0\r\n\r\n[HitObjects]\r\n",
            self.audio,
            self.title,
            self.tags,
            self.keys,
            if self.variables.is_empty() {
                String::new()
            } else {
                format!("[Variables]\r\n{}\r\n\r\n", self.variables.join("\r\n"))
            },
            self.events.join("\r\n"),
        );
        for (col, t) in &self.taps {
            text.push_str(&format!(
                "{},192,{t},1,0,0:0:0:0:{}\r\n",
                x(*col),
                self.tap_sample_file
            ));
        }
        for (col, head, tail) in &self.holds {
            text.push_str(&format!("{},192,{head},128,0,{tail}:0:0:0:0:\r\n", x(*col)));
        }
        text.into_bytes()
    }
}

/// 16-bit mono PCM WAV of a quiet tone, built in memory.
pub(super) fn wav(seconds: f64) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let frames = (seconds * f64::from(RATE)) as usize;
    let data_len = frames * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&u32::try_from(36 + data_len).unwrap().to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&u32::try_from(data_len).unwrap().to_le_bytes());
    // A 441 Hz square wave: libm-free (D3 lints) and not silence.
    const HALF_PERIOD: usize = 25;
    for f in 0..frames {
        let v: i16 = if (f / HALF_PERIOD).is_multiple_of(2) {
            8_000
        } else {
            -8_000
        };
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

//! Rate-copy DTOs (ADR 0025 IPC). camelCase on the wire.

use serde::{Deserialize, Serialize};

/// What `confirm(previewId)` would write. `refusal` set means nothing can be written: the
/// `previewId` is then empty and the names are empty when the chart could not be rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RateCopyPlanDto {
    pub preview_id: String,
    pub md5: String,
    pub rate_milli: u16,
    /// The set folder the files go into.
    pub folder: String,
    pub osu_filename: String,
    pub version: String,
    pub audio_filename: String,
    /// Reused instead of rendered again.
    pub audio_exists: bool,
    /// Skipped: an existing file is never replaced.
    pub osu_exists: bool,
    /// One of [`refusal`]; the UI localises it (`rateCopy.refusal.<id>`).
    pub refusal: Option<String>,
}

/// The error a shell raises when asked to create a refused plan; `args.refusal` names it.
pub const REFUSED_KEY: &str = "rate_copy.error.refused";

/// The job found the copy's audio name taken by something it cannot reuse: a folder, a link,
/// an empty or non-Vorbis file. It is never replaced, so the copy fails.
pub const AUDIO_TARGET_UNUSABLE_KEY: &str = "rate_copy.error.audio_target_unusable";

/// Stable refusal ids, never renamed (§11).
pub mod refusal {
    pub const UNSUPPORTED_MODE: &str = "unsupported_mode";
    /// Per-note sample files, or storyboard `Sample` events in the `.osu` or the set's `.osb`.
    pub const KEYSOUNDED: &str = "keysounded";
    pub const NO_AUDIO: &str = "no_audio";
    /// The chart names an audio file the set folder does not hold.
    pub const AUDIO_MISSING: &str = "audio_missing";
    pub const MALFORMED: &str = "malformed";
    pub const RATE_OUT_OF_RANGE: &str = "rate_out_of_range";
    pub const IDENTITY_RATE: &str = "identity_rate";
    pub const ALREADY_RATE_COPY: &str = "already_rate_copy";
    pub const LN_HEAVY: &str = "ln_heavy";
    /// Rounding at this rate puts two objects of one column within the collision window.
    pub const SAME_COLUMN_COLLISION: &str = "same_column_collision";
    /// The copy's names are not plain file names of one folder that stable can open.
    pub const UNSAFE_NAME: &str = "unsafe_name";
    /// The copy's `.osu` name is taken by something that does not play the copy's audio.
    pub const ALREADY_EXISTS: &str = "already_exists";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_wire_shape() {
        let dto = RateCopyPlanDto {
            preview_id: "01J".to_owned(),
            md5: "0".repeat(32),
            rate_milli: 1150,
            folder: "/s/1 a".to_owned(),
            osu_filename: "a.osu".to_owned(),
            version: "x 1.15x (138bpm)".to_owned(),
            audio_filename: "audio 1.15x.ogg".to_owned(),
            audio_exists: false,
            osu_exists: true,
            refusal: Some(refusal::LN_HEAVY.to_owned()),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(
            json,
            format!(
                r#"{{"previewId":"01J","md5":"{}","rateMilli":1150,"folder":"/s/1 a","osuFilename":"a.osu","version":"x 1.15x (138bpm)","audioFilename":"audio 1.15x.ogg","audioExists":false,"osuExists":true,"refusal":"ln_heavy"}}"#,
                "0".repeat(32)
            )
        );
    }
}

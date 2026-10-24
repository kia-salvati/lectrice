use serde::{Deserialize, Serialize};

use crate::ids::{DocumentId, SentenceId, VoiceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: VoiceId,
    pub name: String,
    pub language: VoiceLanguage,
    pub gender: VoiceGender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoiceLanguage {
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "en-GB")]
    EnGb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceGender {
    Female,
    Male,
}

/// Speech is always made at 1x; the player applies the listener's speed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechRequest {
    pub text: String,
    pub voice_id: VoiceId,
}

/// Mono PCM samples in -1.0..=1.0.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// One cached sentence clip. The cleaner version is part of the key because re-cleaning renumbers sentences.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioKey {
    pub document_id: DocumentId,
    pub cleaner_version: u32,
    pub engine_id: String,
    pub voice_id: VoiceId,
    pub sentence_id: SentenceId,
}

/// One stored voice sample. Raising `version` (when the sample text changes) makes older samples unused.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VoiceSampleKey {
    pub engine_id: String,
    pub voice_id: VoiceId,
    pub version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn voice_serializes_like_spec_section_7() {
        let voice = Voice { id: VoiceId::new("af_heart"), name: "Heart".into(), language: VoiceLanguage::EnUs, gender: VoiceGender::Female };
        let expected = json!({ "id": "af_heart", "name": "Heart", "language": "en-US", "gender": "female" });
        assert_eq!(serde_json::to_value(voice).unwrap(), expected);
    }
}

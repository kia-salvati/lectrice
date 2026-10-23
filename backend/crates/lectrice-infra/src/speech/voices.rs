use lectrice_domain::{Voice, VoiceGender, VoiceId, VoiceLanguage};

use VoiceGender::{Female, Male};
use VoiceLanguage::{EnGb, EnUs};

struct KokoroVoice {
    key: &'static str,
    name: &'static str,
    language: VoiceLanguage,
    gender: VoiceGender,
}

const fn voice(key: &'static str, name: &'static str, language: VoiceLanguage, gender: VoiceGender) -> KokoroVoice {
    KokoroVoice { key, name, language, gender }
}

/// The English voices (speaker ids 0–27 of the bundle's 54), in speaker-id order: a voice's index is its sherpa-onnx `sid`.
/// Taken from the `speaker2id` metadata in `model.onnx`.
const ENGLISH_VOICES: [KokoroVoice; 28] = [
    voice("af_alloy", "Alloy", EnUs, Female),
    voice("af_aoede", "Aoede", EnUs, Female),
    voice("af_bella", "Bella", EnUs, Female),
    voice("af_heart", "Heart", EnUs, Female),
    voice("af_jessica", "Jessica", EnUs, Female),
    voice("af_kore", "Kore", EnUs, Female),
    voice("af_nicole", "Nicole", EnUs, Female),
    voice("af_nova", "Nova", EnUs, Female),
    voice("af_river", "River", EnUs, Female),
    voice("af_sarah", "Sarah", EnUs, Female),
    voice("af_sky", "Sky", EnUs, Female),
    voice("am_adam", "Adam", EnUs, Male),
    voice("am_echo", "Echo", EnUs, Male),
    voice("am_eric", "Eric", EnUs, Male),
    voice("am_fenrir", "Fenrir", EnUs, Male),
    voice("am_liam", "Liam", EnUs, Male),
    voice("am_michael", "Michael", EnUs, Male),
    voice("am_onyx", "Onyx", EnUs, Male),
    voice("am_puck", "Puck", EnUs, Male),
    voice("am_santa", "Santa", EnUs, Male),
    voice("bf_alice", "Alice", EnGb, Female),
    voice("bf_emma", "Emma", EnGb, Female),
    voice("bf_isabella", "Isabella", EnGb, Female),
    voice("bf_lily", "Lily", EnGb, Female),
    voice("bm_daniel", "Daniel", EnGb, Male),
    voice("bm_fable", "Fable", EnGb, Male),
    voice("bm_george", "George", EnGb, Male),
    voice("bm_lewis", "Lewis", EnGb, Male),
];

pub(super) fn speaker_id(id: &VoiceId) -> Option<i32> {
    let index = ENGLISH_VOICES.iter().position(|voice| voice.key == id.as_str())?;
    i32::try_from(index).ok()
}

pub(super) fn count() -> usize {
    ENGLISH_VOICES.len()
}

pub(super) fn all() -> Vec<Voice> {
    ENGLISH_VOICES
        .iter()
        .map(|voice| Voice { id: VoiceId::new(voice.key), name: voice.name.to_owned(), language: voice.language, gender: voice.gender })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const EXPOSED_VOICE_COUNT: usize = 28;

    #[test]
    fn exposes_the_28_english_voices_with_unique_ids_and_names() {
        let voices = all();
        assert_eq!(voices.len(), EXPOSED_VOICE_COUNT);
        assert_eq!(voices.iter().map(|voice| voice.id.as_str()).collect::<HashSet<_>>().len(), EXPOSED_VOICE_COUNT);
        assert_eq!(voices.iter().map(|voice| voice.name.as_str()).collect::<HashSet<_>>().len(), EXPOSED_VOICE_COUNT);
    }

    #[test]
    fn language_and_gender_follow_the_kokoro_prefix() {
        for voice in all() {
            let mut prefix = voice.id.as_str().chars();
            let expected_language = if prefix.next() == Some('a') { EnUs } else { EnGb };
            let expected_gender = if prefix.next() == Some('f') { Female } else { Male };
            assert_eq!((voice.language, voice.gender), (expected_language, expected_gender), "{}", voice.id);
        }
    }

    #[test]
    fn maps_names_to_the_model_speaker_ids() {
        let heart = all().into_iter().find(|voice| voice.id.as_str() == "af_heart").unwrap();
        assert_eq!(heart, Voice { id: VoiceId::new("af_heart"), name: "Heart".into(), language: EnUs, gender: Female });
        assert_eq!(speaker_id(&VoiceId::new("af_alloy")), Some(0));
        assert_eq!(speaker_id(&VoiceId::new("af_heart")), Some(3));
        assert_eq!(speaker_id(&VoiceId::new("bf_alice")), Some(20));
        assert_eq!(speaker_id(&VoiceId::new("bm_lewis")), Some(27));
    }

    #[test]
    fn non_english_and_unknown_voices_have_no_speaker_id() {
        for id in ["ef_dora", "em_santa", "zf_xiaobei", "", "AF_HEART", "nobody"] {
            assert_eq!(speaker_id(&VoiceId::new(id)), None, "{id}");
        }
    }
}

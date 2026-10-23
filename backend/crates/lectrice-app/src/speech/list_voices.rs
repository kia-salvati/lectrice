use std::sync::Arc;

use lectrice_domain::{AppResult, SpeechEngine, Voice, VoiceId};
use tracing::info;

use crate::{RequestContext, UseCase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceList {
    pub voices: Vec<Voice>,
    pub default_voice_id: VoiceId,
}

/// The engine's voices plus the configured default voice.
pub struct ListVoices {
    engine: Arc<dyn SpeechEngine>,
    default_voice_id: VoiceId,
}

impl ListVoices {
    pub fn new(engine: Arc<dyn SpeechEngine>, default_voice_id: VoiceId) -> Self {
        Self { engine, default_voice_id }
    }
}

impl UseCase for ListVoices {
    const NAME: &'static str = "ListVoices";
    type Args = ();
    type Output = VoiceList;

    fn run(&self, ctx: &RequestContext, _args: ()) -> AppResult<VoiceList> {
        info!(service = Self::NAME, request_id = %ctx.request_id, "listing voices");

        let voices = self.engine.voices();
        info!(service = Self::NAME, engine_id = self.engine.id(), voices = voices.len(), default_voice_id = %self.default_voice_id, "voices listed");
        Ok(VoiceList { voices, default_voice_id: self.default_voice_id.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::fakes::GatedEngine;

    #[test]
    fn returns_the_engine_voices_and_the_default() {
        let service = ListVoices::new(Arc::new(GatedEngine::open()), VoiceId::new("af_heart"));

        let list = service.run(&RequestContext::new("test"), ()).unwrap();

        assert_eq!(list.voices.len(), 2);
        assert_eq!(list.default_voice_id, VoiceId::new("af_heart"));
    }
}

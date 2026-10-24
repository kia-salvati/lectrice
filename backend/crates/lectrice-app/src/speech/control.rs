//! What a shell controls while it runs: stopping the read-ahead, its length, and how fast the engine is.
//! When to call them (pause, battery, heat) is the shell's rule.

use std::sync::Arc;

use lectrice_domain::{AppResult, DocumentId, SpeechEngine};
use tracing::info;

use super::read_ahead::ReadAhead;
use super::worker::SpeechWorker;
use crate::{RequestContext, UseCase};

/// Cancels one document's queued read-ahead (every voice and engine); the clip being made is still cached. Returns
/// how many jobs were cancelled. Only the next planning request starts it again.
pub struct StopReadAhead {
    read_ahead: Arc<ReadAhead>,
}

impl StopReadAhead {
    pub fn new(read_ahead: Arc<ReadAhead>) -> Self {
        Self { read_ahead }
    }
}

impl UseCase for StopReadAhead {
    const NAME: &'static str = "StopReadAhead";
    type Args = DocumentId;
    type Output = usize;

    fn run(&self, ctx: &RequestContext, document_id: DocumentId) -> AppResult<usize> {
        let cancelled = self.read_ahead.stop(&document_id);
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %document_id, cancelled, "read-ahead stopped");
        Ok(cancelled)
    }
}

/// [`StopReadAhead`] for every document (e.g. before the engine is swapped).
pub struct StopAllReadAhead {
    read_ahead: Arc<ReadAhead>,
}

impl StopAllReadAhead {
    pub fn new(read_ahead: Arc<ReadAhead>) -> Self {
        Self { read_ahead }
    }
}

impl UseCase for StopAllReadAhead {
    const NAME: &'static str = "StopAllReadAhead";
    type Args = ();
    type Output = usize;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<usize> {
        let cancelled = self.read_ahead.stop_all();
        info!(service = Self::NAME, request_id = %ctx.request_id, cancelled, "all read-ahead stopped");
        Ok(cancelled)
    }
}

/// Listening minutes to make ahead, 1 to 60 and within the cache window; used from the next planning request on.
pub struct SetReadAheadMinutes {
    read_ahead: Arc<ReadAhead>,
}

impl SetReadAheadMinutes {
    pub fn new(read_ahead: Arc<ReadAhead>) -> Self {
        Self { read_ahead }
    }
}

impl UseCase for SetReadAheadMinutes {
    const NAME: &'static str = "SetReadAheadMinutes";
    type Args = u32;
    type Output = ();

    fn run(&self, ctx: &RequestContext, minutes: u32) -> AppResult<()> {
        self.read_ahead.set_read_ahead_minutes(minutes)?;
        info!(service = Self::NAME, request_id = %ctx.request_id, minutes, "read-ahead minutes set");
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechStats {
    pub engine_id: String,
    /// Synthesis time ÷ audio length, the median of the last 50 synthesized sentences; `None` below 10.
    pub rtf_median: Option<f32>,
    pub samples: u32,
    /// How long the shell took to load the engine, when it measured it.
    pub engine_load_ms: Option<u32>,
}

pub struct GetSpeechStats {
    engine: Arc<dyn SpeechEngine>,
    worker: Arc<SpeechWorker>,
    engine_load_ms: Option<u32>,
}

impl GetSpeechStats {
    pub fn new(engine: Arc<dyn SpeechEngine>, worker: Arc<SpeechWorker>, engine_load_ms: Option<u32>) -> Self {
        Self { engine, worker, engine_load_ms }
    }
}

impl UseCase for GetSpeechStats {
    const NAME: &'static str = "GetSpeechStats";
    type Args = ();
    type Output = SpeechStats;

    fn run(&self, ctx: &RequestContext, (): ()) -> AppResult<SpeechStats> {
        let timings = self.worker.timings();
        info!(service = Self::NAME, request_id = %ctx.request_id, samples = timings.samples, rtf_median = timings.rtf_median, "speech stats read");
        Ok(SpeechStats {
            engine_id: self.engine.id().to_string(),
            rtf_median: timings.rtf_median,
            samples: timings.samples,
            engine_load_ms: self.engine_load_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::fakes::GatedEngine;

    #[test]
    fn stats_name_the_engine_and_the_load_time() {
        let engine = Arc::new(GatedEngine::open());
        let worker = Arc::new(SpeechWorker::start(engine.clone()).unwrap());

        let stats = GetSpeechStats::new(engine, worker, Some(1200)).run(&RequestContext::new("test"), ()).unwrap();

        assert_eq!(stats, SpeechStats { engine_id: "fake-engine".into(), rtf_median: None, samples: 0, engine_load_ms: Some(1200) });
    }
}

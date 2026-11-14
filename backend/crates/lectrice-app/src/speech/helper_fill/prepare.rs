//! A Prepare range the computer makes after the listener's plan, in playback order.

use std::collections::HashSet;

use lectrice_domain::{AppError, AppResult, AudioKey, DocumentId, ErrorName, SentenceId};
use tracing::info;

use super::{FillState, HelperFill, NAME, Planned, estimated_ms};

/// A Prepare range (C21) the helper makes after the listener's plan, in playback order.
pub(super) struct PrepareTarget {
    /// Book, cleaner version, engine and voice of the range (its `sentence_id` means nothing).
    pub(super) scope: AudioKey,
    pub(super) plan: Vec<Planned>,
    pub(super) failed: HashSet<SentenceId>,
}

/// How far a Prepare range made by the helper has come, counted in its playable sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HelperPrepareProgress {
    pub ready: u32,
    pub total: u32,
    /// Some sentences are still to be made (not cached, not refused by the helper).
    pub running: bool,
}

impl HelperFill {
    /// Has the helper make these sentences of one book (a Prepare range) after the listener's plan,
    /// replacing any earlier range. `scope` names the book, cleaner version, engine and voice. Survives a lost link:
    /// the range continues once the helper is back. `HelperUnreachable` when the helper can't make clips now.
    pub fn prepare(&self, scope: AudioKey, sentences: Vec<(SentenceId, String)>) -> AppResult<HelperPrepareProgress> {
        let mut state = self.lock();
        if !state.link.makes_clips() {
            return Err(AppError::new(ErrorName::HelperUnreachable).with_detail("the computer isn't connected"));
        }
        if let Some(previous) = state.prepare.as_ref().map(|prepare| prepare.scope.document_id.clone()) {
            self.drop_prepare(&mut state, &previous);
        }
        let plan = sentences.into_iter().map(|(sentence_id, text)| Planned { sentence_id, estimated_ms: estimated_ms(&text), text }).collect();
        info!(service = NAME, doc_id = %scope.document_id, voice_id = %scope.voice_id, "helper prepare started");
        state.prepare = Some(PrepareTarget { scope, plan, failed: HashSet::new() });
        self.top_up(&mut state);
        Ok(self.progress_of(&state))
    }

    /// Stops the document's Prepare range on the helper (what was made stays cached). `false` when it had none.
    pub fn stop_prepare(&self, document_id: &DocumentId) -> bool {
        self.drop_prepare(&mut self.lock(), document_id)
    }

    /// The document's Prepare range on the helper, if it has one.
    pub fn prepare_progress(&self, document_id: &DocumentId) -> Option<HelperPrepareProgress> {
        let state = self.lock();
        state.prepare.as_ref().filter(|prepare| prepare.scope.document_id == *document_id)?;
        Some(self.progress_of(&state))
    }

    pub(super) fn progress_of(&self, state: &FillState) -> HelperPrepareProgress {
        let Some(prepare) = state.prepare.as_ref() else { return HelperPrepareProgress::default() };
        let ready = prepare
            .plan
            .iter()
            .filter(|planned| {
                let key = AudioKey { sentence_id: planned.sentence_id, ..prepare.scope.clone() };
                !state.storing.contains(&key) && self.deps.retention.contains(&key).unwrap_or(false)
            })
            .count();
        let total = prepare.plan.len();
        let refused = prepare.failed.len();
        HelperPrepareProgress {
            ready: u32::try_from(ready).unwrap_or(u32::MAX),
            total: u32::try_from(total).unwrap_or(u32::MAX),
            running: ready + refused < total,
        }
    }

    pub(super) fn drop_prepare(&self, state: &mut FillState, document_id: &DocumentId) -> bool {
        if state.prepare.as_ref().is_none_or(|prepare| prepare.scope.document_id != *document_id) {
            return false;
        }
        state.prepare = None;
        let dropped: Vec<u32> = state.in_flight.iter().filter(|(_, flight)| flight.prepare).map(|(item_id, _)| *item_id).collect();
        if !dropped.is_empty() {
            // The helper can only drop its whole queue: the listener's items are asked for again below.
            self.cancel_on_helper(state);
            state.in_flight.retain(|_, flight| !flight.prepare);
            self.resend_in_flight(state);
        }
        info!(service = NAME, doc_id = %document_id, cancelled = dropped.len(), "helper prepare stopped");
        true
    }
}

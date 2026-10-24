//! The read-ahead planner (spec §4): which sentences to make next, in playback order. Pure, no I/O.

use lectrice_domain::{Block, Document, Sentence, SentenceId};

/// Speaking rate at 1x (spec §4, measured): text length → estimated audio length.
pub(crate) const CHARS_PER_SECOND: f64 = 18.0;
pub(crate) const SECONDS_PER_MINUTE: f64 = 60.0;
pub(crate) const MILLIS_PER_SECOND: f64 = 1000.0;

/// Estimated 1x audio of `chars` spoken characters, in whole milliseconds.
pub(crate) fn chars_to_ms(chars: u64) -> u64 {
    (chars as f64 / CHARS_PER_SECOND * MILLIS_PER_SECOND).round() as u64
}

#[derive(Debug, Clone, Copy)]
pub struct ReadAheadInput<'a> {
    pub document: &'a Document,
    /// The sentence being requested now; the plan starts after it.
    pub from: SentenceId,
    /// The listener's speed: `minutes` of listening needs `minutes × rate` of 1x audio.
    pub rate: f32,
    pub read_skipped: bool,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadAheadPlan<'a> {
    /// In playback order. Sentences with nothing to say (empty speech text) are left out.
    pub sentences: Vec<&'a Sentence>,
    /// Estimated 1x audio of `sentences`.
    pub estimated_seconds: f64,
}

#[cfg(test)]
impl ReadAheadPlan<'_> {
    fn ids(&self) -> Vec<SentenceId> {
        self.sentences.iter().map(|sentence| sentence.id).collect()
    }
}

/// The sentences after `from` that cover `minutes × rate` minutes of 1x audio (the last one may overshoot).
/// A `from` that isn't playable in this order (e.g. code text with `read_skipped` off) starts at the first
/// playable sentence of its block or a later one, which is what the player plays next. An unknown id plans nothing.
pub fn plan_read_ahead(input: ReadAheadInput<'_>) -> ReadAheadPlan<'_> {
    let order = playable_in_order(input.document, input.read_skipped);
    let budget_seconds = f64::from(input.minutes) * SECONDS_PER_MINUTE * f64::from(input.rate);
    let mut plan = ReadAheadPlan { sentences: Vec::new(), estimated_seconds: 0.0 };
    let Some(start) = start_position(input.document, &order, input.from) else {
        return plan;
    };
    for &(_, sentence) in &order[start..] {
        if plan.estimated_seconds >= budget_seconds {
            break;
        }
        let text = sentence.spoken_text();
        if text.trim().is_empty() {
            continue;
        }
        plan.estimated_seconds += estimated_seconds(text);
        plan.sentences.push(sentence);
    }
    plan
}

/// Estimated 1x audio of the sentences right after `from` in playback order that are ready, up to the first one that
/// isn't (sentences with nothing to say are always ready).
pub(crate) fn ready_ahead_seconds(document: &Document, from: SentenceId, read_skipped: bool, mut is_cached: impl FnMut(SentenceId) -> bool) -> f64 {
    let order = playable_in_order(document, read_skipped);
    let Some(start) = start_position(document, &order, from) else {
        return 0.0;
    };
    let mut seconds = 0.0;
    for &(_, sentence) in &order[start..] {
        let text = sentence.spoken_text();
        if text.trim().is_empty() {
            continue;
        }
        if !is_cached(sentence.id) {
            break;
        }
        seconds += estimated_seconds(text);
    }
    seconds
}

fn estimated_seconds(text: &str) -> f64 {
    text.chars().count() as f64 / CHARS_PER_SECOND
}

/// The ids of what playback reads, in order: the one source for a player's playlist, the read-ahead and exports.
pub fn playback_order(document: &Document, read_skipped: bool) -> Vec<SentenceId> {
    playable_in_order(document, read_skipped).into_iter().map(|(_, sentence)| sentence.id).collect()
}

/// What playback reads, in order, with each sentence's block index. Mirrors the frontend's
/// `buildPlaybackOrder` (`frontend/src/utils/playbackOrder.ts`): a skipped block (code, table, caption)
/// plays its cue when `read_skipped` is off (captions have none) and its own sentences when it's on.
pub(crate) fn playable_in_order(document: &Document, read_skipped: bool) -> Vec<(usize, &Sentence)> {
    document.blocks.iter().enumerate().flat_map(|(index, block)| playable_sentences(block, read_skipped).map(move |sentence| (index, sentence))).collect()
}

fn playable_sentences(block: &Block, read_skipped: bool) -> Box<dyn Iterator<Item = &Sentence> + '_> {
    if block.spoken || read_skipped {
        return Box::new(block.sentences.iter());
    }
    Box::new(block.cue.iter())
}

fn start_position(document: &Document, order: &[(usize, &Sentence)], from: SentenceId) -> Option<usize> {
    if let Some(position) = order.iter().position(|(_, sentence)| sentence.id == from) {
        return Some(position + 1);
    }
    let block_index = document.blocks.iter().position(|block| block.sentences.iter().chain(block.cue.iter()).any(|sentence| sentence.id == from))?;
    Some(order.iter().position(|&(index, _)| index >= block_index).unwrap_or(order.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lectrice_domain::{BlockKind, DocumentId};

    fn sentence(id: u32, text: &str) -> Sentence {
        Sentence { id: SentenceId(id), text: text.to_string(), page_index: 1, speech_text: None }
    }

    fn block(index: u32, kind: BlockKind, sentences: Vec<Sentence>, cue: Option<Sentence>) -> Block {
        let spoken = kind.is_spoken_by_default();
        let text = (!spoken).then(|| "shown".to_string());
        Block { id: index, kind, level: None, page_index: 1, spoken, text, sentences, cue }
    }

    fn document(blocks: Vec<Block>) -> Document {
        Document {
            id: DocumentId::parse(&format!("{:064x}", 1)).unwrap(),
            title: "T".into(),
            source_file_name: "t.pdf".into(),
            page_count: 1,
            cleaner_version: 1,
            pages: Vec::new(),
            toc: Vec::new(),
            blocks,
        }
    }

    /// A heading, a paragraph, code with a cue, a caption (no cue), a table with a cue, a last paragraph.
    fn mixed_document() -> Document {
        document(vec![
            block(0, BlockKind::Heading, vec![sentence(0, "Intro")], None),
            block(1, BlockKind::Paragraph, vec![sentence(1, "One."), sentence(2, "Two.")], None),
            block(2, BlockKind::Code, vec![sentence(3, "let x = 1;"), sentence(4, "let y = 2;")], Some(sentence(5, "Code example."))),
            block(3, BlockKind::Caption, vec![sentence(6, "Figure 1. A box.")], None),
            block(4, BlockKind::Table, vec![sentence(7, "a b c")], Some(sentence(8, "Table."))),
            block(5, BlockKind::Paragraph, vec![sentence(9, "End.")], None),
        ])
    }

    fn order_ids(document: &Document, read_skipped: bool) -> Vec<u32> {
        playback_order(document, read_skipped).iter().map(|id| id.0).collect()
    }

    fn plan_ids(document: &Document, from: u32, read_skipped: bool) -> Vec<u32> {
        let input = ReadAheadInput { document, from: SentenceId(from), rate: 1.0, read_skipped, minutes: 10 };
        plan_read_ahead(input).ids().iter().map(|id| id.0).collect()
    }

    /// One paragraph per sentence, each `seconds` long at 1x.
    fn uniform_document(count: u32, seconds: usize) -> Document {
        let text = "x".repeat(seconds * CHARS_PER_SECOND as usize);
        document((0..count).map(|id| block(id, BlockKind::Paragraph, vec![sentence(id, &text)], None)).collect())
    }

    #[test]
    fn skipped_blocks_play_their_cue_when_read_skipped_is_off() {
        assert_eq!(order_ids(&mixed_document(), false), [0, 1, 2, 5, 8, 9]);
    }

    #[test]
    fn skipped_blocks_play_their_own_sentences_and_never_the_cue_when_read_skipped_is_on() {
        assert_eq!(order_ids(&mixed_document(), true), [0, 1, 2, 3, 4, 6, 7, 9]);
    }

    #[test]
    fn the_plan_starts_after_the_requested_sentence_in_playback_order() {
        let document = mixed_document();
        assert_eq!(plan_ids(&document, 1, false), [2, 5, 8, 9]);
        assert_eq!(plan_ids(&document, 2, true), [3, 4, 6, 7, 9]);
        assert!(plan_ids(&document, 9, false).is_empty());
    }

    #[test]
    fn a_sentence_that_is_not_playable_starts_at_the_next_playable_one_from_its_block() {
        let document = mixed_document();
        assert_eq!(plan_ids(&document, 3, false), [5, 8, 9]);
        assert_eq!(plan_ids(&document, 6, false), [8, 9]);
        assert_eq!(plan_ids(&document, 5, true), [3, 4, 6, 7, 9]);
        assert!(plan_ids(&document, 99, false).is_empty());
    }

    #[test]
    fn at_1x_the_plan_covers_the_read_ahead_minutes() {
        let document = uniform_document(200, 10);
        let input = ReadAheadInput { document: &document, from: SentenceId(0), rate: 1.0, read_skipped: false, minutes: 10 };

        let plan = plan_read_ahead(input);

        assert_eq!(plan.sentences.len(), 60);
        assert!((plan.estimated_seconds - 600.0).abs() < 1e-9);
        assert_eq!(plan.sentences[0].id, SentenceId(1));
    }

    #[test]
    fn at_2x_the_plan_covers_twice_the_audio() {
        let document = uniform_document(200, 10);
        let at = |rate| plan_read_ahead(ReadAheadInput { document: &document, from: SentenceId(0), rate, read_skipped: false, minutes: 10 });

        let (single, double) = (at(1.0), at(2.0));

        assert_eq!(double.sentences.len(), 2 * single.sentences.len());
        assert!((double.estimated_seconds - 2.0 * single.estimated_seconds).abs() < 1e-9);
        assert!((at(0.75).estimated_seconds - 450.0).abs() < 1e-9);
    }

    #[test]
    fn the_last_sentence_may_overshoot_and_the_plan_stops_at_the_end() {
        let document = uniform_document(5, 400);
        let plan = plan_read_ahead(ReadAheadInput { document: &document, from: SentenceId(0), rate: 1.0, read_skipped: false, minutes: 10 });
        assert_eq!(plan.ids(), [SentenceId(1), SentenceId(2)]);

        let plan = plan_read_ahead(ReadAheadInput { document: &document, from: SentenceId(3), rate: 2.0, read_skipped: false, minutes: 10 });
        assert_eq!(plan.ids(), [SentenceId(4)]);
    }

    /// C25: only the unbroken run of cached sentences after the current one counts.
    #[test]
    fn ready_ahead_counts_the_cached_run_after_the_sentence() {
        let document = uniform_document(10, 2);
        let cached = [1, 2, 3, 5];

        let seconds = ready_ahead_seconds(&document, SentenceId(0), false, |id| cached.contains(&id.0));

        assert!((seconds - 6.0).abs() < 1e-9);
        assert!(ready_ahead_seconds(&document, SentenceId(3), false, |id| cached.contains(&id.0)).abs() < 1e-9);
    }

    #[test]
    fn speech_text_sizes_the_estimate_and_empty_speech_text_is_left_out() {
        let mut cited = sentence(1, "A long sentence with a citation [12] in it.");
        cited.speech_text = Some("Short.".into());
        let mut url_only = sentence(2, "https://example.com");
        url_only.speech_text = Some(String::new());
        let document = document(vec![block(0, BlockKind::Paragraph, vec![sentence(0, "Start."), cited, url_only, sentence(3, "End.")], None)]);

        let plan = plan_read_ahead(ReadAheadInput { document: &document, from: SentenceId(0), rate: 1.0, read_skipped: false, minutes: 10 });

        assert_eq!(plan.ids(), [SentenceId(1), SentenceId(3)]);
        assert!((plan.estimated_seconds - ("Short.".len() + "End.".len()) as f64 / CHARS_PER_SECOND).abs() < 1e-9);
    }
}

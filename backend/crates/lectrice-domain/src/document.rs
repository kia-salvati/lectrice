use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::ids::{BlockId, DocumentId, SentenceId};

const CODE_CUE: &str = "Code example.";
const TABLE_CUE: &str = "Table.";

/// The cleaned document, saved as `document.json` (spec §5) and sent to the reader as is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: DocumentId,
    pub title: String,
    pub source_file_name: String,
    pub page_count: u32,
    pub cleaner_version: u32,
    pub pages: Vec<PageLabel>,
    pub blocks: Vec<Block>,
}

impl Document {
    /// Sentence ids run from 0 without gaps, cue sentences included.
    pub fn sentence_count(&self) -> u32 {
        let count: usize = self.blocks.iter().map(|block| block.sentences.len() + usize::from(block.cue.is_some())).sum();
        u32::try_from(count).unwrap_or(u32::MAX)
    }
}

/// Maps one PDF page (1-based) to the label printed on it ("xii", "37"). Never spoken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLabel {
    pub index: u32,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
    Heading,
    Paragraph,
    ListItem,
    Note,
    Code,
    Table,
    Caption,
}

impl BlockKind {
    /// Code, tables and captions are shown but not read unless the listener turns "read aloud" on.
    pub const fn is_spoken_by_default(self) -> bool {
        !matches!(self, Self::Code | Self::Table | Self::Caption)
    }

    /// The short stand-in playback says once instead of a skipped block.
    pub const fn cue_text(self) -> Option<&'static str> {
        match self {
            Self::Code => Some(CODE_CUE),
            Self::Table => Some(TABLE_CUE),
            _ => None,
        }
    }
}

/// `text` is set for skipped kinds (display text with its line breaks); `cue` only for code and tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    pub page_index: u32,
    #[serde(default = "spoken_when_absent", skip_serializing_if = "is_spoken")]
    pub spoken: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub sentences: Vec<Sentence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue: Option<Sentence>,
}

fn spoken_when_absent() -> bool {
    true
}

fn is_spoken(spoken: &bool) -> bool {
    *spoken
}

/// `speech_text` is what the engine reads when it differs from the shown `text`
/// (citation markers, URLs and emoji removed); `None` means read `text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sentence {
    pub id: SentenceId,
    pub text: String,
    pub page_index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_text: Option<String>,
}

impl Sentence {
    pub fn spoken_text(&self) -> &str {
        self.speech_text.as_deref().unwrap_or(&self.text)
    }
}

/// Library bookkeeping kept next to a document (`meta.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    pub added_at: Timestamp,
    pub last_opened_at: Timestamp,
    pub last_sentence_id: Option<SentenceId>,
}

impl DocumentMeta {
    /// What to store when a document is saved again while `saved` is on disk: re-cleaning takes seconds, and a
    /// position or an open recorded meanwhile must not be lost (review L5). The saved position wins if it still
    /// names a sentence of the new document (`sentence_count`); the earliest add and the latest open are kept.
    pub fn merged_with_saved(self, saved: &DocumentMeta, sentence_count: u32) -> DocumentMeta {
        let saved_position = saved.last_sentence_id.filter(|id| id.0 < sentence_count);
        DocumentMeta {
            added_at: self.added_at.min(saved.added_at),
            last_opened_at: self.last_opened_at.max(saved.last_opened_at),
            last_sentence_id: saved_position.or(self.last_sentence_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadingPosition {
    pub sentence_id: SentenceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: DocumentId,
    pub title: String,
    pub source_file_name: String,
    pub page_count: u32,
    pub sentence_count: u32,
    pub added_at: Timestamp,
    pub last_opened_at: Timestamp,
    pub last_sentence_id: Option<SentenceId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sentence(id: u32, text: &str) -> Sentence {
        Sentence { id: SentenceId(id), text: text.to_string(), page_index: 26, speech_text: None }
    }

    #[test]
    fn spoken_block_serializes_like_spec_section_5() {
        let block =
            Block { id: 1, kind: BlockKind::Paragraph, level: None, page_index: 25, spoken: true, text: None, sentences: vec![sentence(1, "Hi.")], cue: None };
        let expected = json!({ "id": 1, "kind": "paragraph", "pageIndex": 25, "sentences": [{ "id": 1, "text": "Hi.", "pageIndex": 26 }] });
        assert_eq!(serde_json::to_value(&block).unwrap(), expected);
    }

    #[test]
    fn skipped_block_carries_spoken_false_text_and_cue() {
        let block = Block {
            id: 2,
            kind: BlockKind::Code,
            level: None,
            page_index: 26,
            spoken: false,
            text: Some("fn x() {}".into()),
            sentences: vec![sentence(811, "fn x() {}")],
            cue: Some(sentence(812, "Code example.")),
        };
        let value = serde_json::to_value(&block).unwrap();
        assert_eq!(value["kind"], "code");
        assert_eq!(value["spoken"], false);
        assert_eq!(value["cue"]["id"], 812);
        assert_eq!(serde_json::from_value::<Block>(value).unwrap(), block);
    }

    #[test]
    fn list_item_kind_is_camel_case() {
        assert_eq!(serde_json::to_value(BlockKind::ListItem).unwrap(), "listItem");
    }

    #[test]
    fn only_code_and_tables_have_cues() {
        assert_eq!(BlockKind::Code.cue_text(), Some(CODE_CUE));
        assert_eq!(BlockKind::Table.cue_text(), Some(TABLE_CUE));
        assert_eq!(BlockKind::Caption.cue_text(), None);
        assert!(!BlockKind::Caption.is_spoken_by_default());
        assert!(BlockKind::Note.is_spoken_by_default());
    }

    #[test]
    fn spoken_text_falls_back_to_shown_text() {
        let mut shown = sentence(3, "See [12] for more.");
        assert_eq!(shown.spoken_text(), "See [12] for more.");
        shown.speech_text = Some("See for more.".into());
        assert_eq!(shown.spoken_text(), "See for more.");
    }
}

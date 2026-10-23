use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, DocumentId, DocumentReader, DocumentWriter, ErrorName, ReadingPosition, SentenceId};
use tracing::info;

use crate::{RequestContext, UseCase};

#[derive(Debug, Clone)]
pub struct SaveReadingPositionArgs {
    pub id: DocumentId,
    pub sentence_id: SentenceId,
}

/// Remembers the last sentence heard, for "continue" and the progress in recent documents.
pub struct SaveReadingPosition {
    reader: Arc<dyn DocumentReader>,
    writer: Arc<dyn DocumentWriter>,
}

impl SaveReadingPosition {
    pub fn new(reader: Arc<dyn DocumentReader>, writer: Arc<dyn DocumentWriter>) -> Self {
        Self { reader, writer }
    }
}

impl UseCase for SaveReadingPosition {
    const NAME: &'static str = "SaveReadingPosition";
    type Args = SaveReadingPositionArgs;
    type Output = ();

    fn run(&self, ctx: &RequestContext, args: SaveReadingPositionArgs) -> AppResult<()> {
        let SaveReadingPositionArgs { id, sentence_id } = args;
        info!(service = Self::NAME, request_id = %ctx.request_id, doc_id = %id, sentence_id = sentence_id.0, "saving reading position");

        let summary = self.reader.find_summary(&id)?;
        info!(service = Self::NAME, doc_id = %id, found = summary.is_some(), sentence_count = summary.as_ref().map(|summary| summary.sentence_count), "library summary looked up");
        let Some(summary) = summary else {
            return Err(AppError::new(ErrorName::DocumentNotFound));
        };
        // Sentence ids run from 0 without gaps, so the count is enough to check one exists.
        if sentence_id.0 >= summary.sentence_count {
            return Err(AppError::new(ErrorName::SentenceNotFound).with_detail(format!("sentence {sentence_id} of {}", summary.sentence_count)));
        }

        self.writer.save_position(&id, ReadingPosition { sentence_id })?;
        info!(service = Self::NAME, doc_id = %id, sentence_id = sentence_id.0, "reading position saved");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::fakes::{Library, id_for};

    fn save(library: &Library, id: DocumentId, sentence_id: u32) -> AppResult<()> {
        let service = SaveReadingPosition::new(library.reader(), library.writer());
        service.run(&RequestContext::new("test"), SaveReadingPositionArgs { id, sentence_id: SentenceId(sentence_id) })
    }

    #[test]
    fn saves_an_existing_sentence() {
        let library = Library::new(1);
        let id = library.add_book();

        save(&library, id.clone(), 4).unwrap();

        assert_eq!(library.store.summary(&id).unwrap().last_sentence_id, Some(SentenceId(4)));
    }

    #[test]
    fn unknown_sentence_is_sentence_not_found() {
        let library = Library::new(1);
        let id = library.add_book();

        let error = save(&library, id.clone(), 5).unwrap_err();

        assert_eq!(error.primary(), ErrorName::SentenceNotFound);
        assert_eq!(library.store.summary(&id).unwrap().last_sentence_id, None);
    }

    #[test]
    fn unknown_document_is_document_not_found() {
        let library = Library::new(1);

        assert_eq!(save(&library, id_for(9), 0).unwrap_err().primary(), ErrorName::DocumentNotFound);
    }
}

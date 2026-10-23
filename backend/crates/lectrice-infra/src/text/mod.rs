//! B2: `SentencexSplitter` plus the extra abbreviation rules, and `Sha256ContentHasher` for document ids.

mod content_hasher;
mod sentencex_splitter;

pub use content_hasher::Sha256ContentHasher;
pub use sentencex_splitter::SentencexSplitter;

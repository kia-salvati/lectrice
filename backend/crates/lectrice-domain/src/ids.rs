use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorName};

const SHA256_HEX_LENGTH: usize = 64;

/// The lowercase hex SHA-256 of the original PDF bytes. Parsing rejects anything else,
/// so an id is always safe to use as a directory name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DocumentId(String);

impl DocumentId {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        let normalized = value.trim().to_ascii_lowercase();
        let is_sha256_hex = normalized.len() == SHA256_HEX_LENGTH && normalized.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !is_sha256_hex {
            return Err(AppError::new(ErrorName::DocumentNotFound).with_detail("document id is not a sha256 hex string"));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DocumentId {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<DocumentId> for String {
    fn from(id: DocumentId) -> Self {
        id.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Global, sequential within one document. Used by the audio route, the highlight and the reading position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SentenceId(pub u32);

impl fmt::Display for SentenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type BlockId = u32;

/// A voice name as the speech engine knows it (e.g. `af_heart`). Only the engine decides if it exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VoiceId(String);

impl VoiceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VoiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExportId(String);

impl ExportId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_ID: &str = "3F2A000000000000000000000000000000000000000000000000000000000abc";

    #[test]
    fn document_id_is_normalized_to_lowercase() {
        let id = DocumentId::parse(VALID_ID).unwrap();
        assert_eq!(id.as_str(), VALID_ID.to_ascii_lowercase());
    }

    #[test]
    fn document_id_rejects_paths_and_short_values() {
        for bad in ["../etc/passwd", "abc", ""] {
            let error = DocumentId::parse(bad).unwrap_err();
            assert_eq!(error.primary(), ErrorName::DocumentNotFound);
        }
    }
}

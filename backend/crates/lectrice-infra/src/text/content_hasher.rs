use std::fmt::Write as _;

use lectrice_domain::{AppResult, ContentHasher, DocumentId};
use sha2::{Digest, Sha256};

/// The document id is the lowercase hex SHA-256 of the PDF bytes.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sha256ContentHasher;

impl ContentHasher for Sha256ContentHasher {
    fn document_id(&self, pdf: &[u8]) -> AppResult<DocumentId> {
        let digest = Sha256::digest(pdf);
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            let _ = write!(hex, "{byte:02x}");
        }
        DocumentId::parse(&hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_the_sha256_hex_of_the_bytes() {
        let id = Sha256ContentHasher.document_id(b"abc").unwrap();

        assert_eq!(id.as_str(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}

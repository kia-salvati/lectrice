use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use lectrice_domain::{
    AppError, AppResult, Document, DocumentBytes, DocumentId, DocumentMeta, DocumentReader, DocumentSummary, DocumentWriter, ErrorName, ListOptions, Page,
    ReadingPosition, SentenceId, Timestamp,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::atomic_file::write_atomically;
use super::storage_failed;

const LIBRARY_DIR: &str = "library";
const ORIGINAL_PDF_FILE: &str = "original.pdf";
const DOCUMENT_FILE: &str = "document.json";
const META_FILE: &str = "meta.json";

/// `meta.json`: the library bookkeeping (spec §6) plus a copy of the summary fields, so listing
/// recent documents never parses a whole `document.json`. It is written last, so a document
/// without it counts as not in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMeta {
    added_at: Timestamp,
    last_opened_at: Timestamp,
    last_sentence_id: Option<SentenceId>,
    title: String,
    source_file_name: String,
    page_count: u32,
    sentence_count: u32,
    /// Missing in files written before v0.2; then it is read from `document.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cleaner_version: Option<u32>,
    /// Set while the document is removed but not deleted yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    removed_at: Option<Timestamp>,
}

impl StoredMeta {
    fn new(document: &Document, meta: &DocumentMeta, removed_at: Option<Timestamp>) -> Self {
        Self {
            added_at: meta.added_at,
            last_opened_at: meta.last_opened_at,
            last_sentence_id: meta.last_sentence_id,
            title: document.title.clone(),
            source_file_name: document.source_file_name.clone(),
            page_count: document.page_count,
            sentence_count: document.sentence_count(),
            cleaner_version: Some(document.cleaner_version),
            removed_at,
        }
    }

    fn meta(&self) -> DocumentMeta {
        DocumentMeta { added_at: self.added_at, last_opened_at: self.last_opened_at, last_sentence_id: self.last_sentence_id }
    }

    fn into_summary(self, id: DocumentId, cleaner_version: u32) -> DocumentSummary {
        DocumentSummary {
            id,
            title: self.title,
            source_file_name: self.source_file_name,
            page_count: self.page_count,
            sentence_count: self.sentence_count,
            cleaner_version,
            added_at: self.added_at,
            last_opened_at: self.last_opened_at,
            last_sentence_id: self.last_sentence_id,
        }
    }
}

/// Reads only the version from `document.json` (the rest is skipped, not built).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredCleanerVersion {
    cleaner_version: u32,
}

/// The library under `<data_dir>/library/<docId>/`: `original.pdf`, `document.json` and `meta.json`.
pub struct FsDocumentRepository {
    library_dir: PathBuf,
    /// Serializes read-modify-write of `meta.json` (opening and position saves can race).
    meta_lock: Mutex<()>,
}

impl FsDocumentRepository {
    pub fn new(data_dir: &Path) -> Self {
        Self { library_dir: data_dir.join(LIBRARY_DIR), meta_lock: Mutex::new(()) }
    }

    fn document_dir(&self, id: &DocumentId) -> PathBuf {
        self.library_dir.join(id.as_str())
    }

    fn lock_meta(&self) -> MutexGuard<'_, ()> {
        self.meta_lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn read_meta(&self, id: &DocumentId) -> AppResult<Option<StoredMeta>> {
        read_json(&self.document_dir(id).join(META_FILE))
    }

    fn update_meta(&self, id: &DocumentId, change: impl FnOnce(&mut StoredMeta)) -> AppResult<()> {
        let _guard = self.lock_meta();
        let Some(mut meta) = self.read_meta(id)? else {
            return Err(AppError::new(ErrorName::DocumentNotFound).with_detail(format!("no {META_FILE} for document {id}")));
        };
        change(&mut meta);
        write_json(&self.document_dir(id).join(META_FILE), &meta)
    }

    /// The meta of a document that isn't removed.
    fn read_listed_meta(&self, id: &DocumentId) -> AppResult<Option<StoredMeta>> {
        Ok(self.read_meta(id)?.filter(|meta| meta.removed_at.is_none()))
    }

    fn summary(&self, id: &DocumentId, meta: StoredMeta) -> AppResult<DocumentSummary> {
        let cleaner_version = match meta.cleaner_version {
            Some(version) => version,
            None => read_json::<StoredCleanerVersion>(&self.document_dir(id).join(DOCUMENT_FILE))?.map_or(0, |stored| stored.cleaner_version),
        };
        Ok(meta.into_summary(id.clone(), cleaner_version))
    }

    /// Every document folder with a `meta.json`, removed ones included.
    fn read_all_metas(&self) -> AppResult<Vec<(DocumentId, StoredMeta)>> {
        let entries = match fs::read_dir(&self.library_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(storage_failed("listing", &self.library_dir, error)),
        };
        let mut metas = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| storage_failed("listing", &self.library_dir, error))?;
            let Some(id) = entry.file_name().to_str().and_then(|name| DocumentId::parse(name).ok()) else {
                continue;
            };
            let is_dir = entry.file_type().map_err(|error| storage_failed("inspecting", &entry.path(), error))?.is_dir();
            if !is_dir {
                continue;
            }
            if let Some(meta) = self.read_meta(&id)? {
                metas.push((id, meta));
            }
        }
        Ok(metas)
    }

    fn document_bytes(&self, id: &DocumentId) -> AppResult<u64> {
        let directory = self.document_dir(id);
        let mut bytes = 0;
        for file in [ORIGINAL_PDF_FILE, DOCUMENT_FILE, META_FILE] {
            let path = directory.join(file);
            match fs::metadata(&path) {
                Ok(metadata) => bytes += metadata.len(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(storage_failed("inspecting", &path, error)),
            }
        }
        Ok(bytes)
    }
}

impl DocumentReader for FsDocumentRepository {
    fn find(&self, id: &DocumentId) -> AppResult<Option<Document>> {
        if self.read_meta(id)?.is_some_and(|meta| meta.removed_at.is_some()) {
            return Ok(None);
        }
        read_json(&self.document_dir(id).join(DOCUMENT_FILE))
    }

    fn find_summary(&self, id: &DocumentId) -> AppResult<Option<DocumentSummary>> {
        self.read_listed_meta(id)?.map(|meta| self.summary(id, meta)).transpose()
    }

    fn find_original_pdf(&self, id: &DocumentId) -> AppResult<Option<Vec<u8>>> {
        read_optional(&self.document_dir(id).join(ORIGINAL_PDF_FILE))
    }

    fn list_recent(&self, options: ListOptions) -> AppResult<Page<DocumentSummary>> {
        let limit = options.limit_or_default();
        let offset = options.offset.unwrap_or(0);
        let mut metas: Vec<(DocumentId, StoredMeta)> = self.read_all_metas()?.into_iter().filter(|(_, meta)| meta.removed_at.is_none()).collect();
        metas.sort_by(|(a_id, a), (b_id, b)| b.last_opened_at.cmp(&a.last_opened_at).then_with(|| a_id.cmp(b_id)));
        let total = metas.len();
        let items = metas.into_iter().skip(offset).take(limit).map(|(id, meta)| self.summary(&id, meta)).collect::<AppResult<Vec<_>>>()?;
        Ok(Page { items, total, limit, offset })
    }

    fn list_removed(&self) -> AppResult<Vec<DocumentId>> {
        Ok(self.read_all_metas()?.into_iter().filter(|(_, meta)| meta.removed_at.is_some()).map(|(id, _)| id).collect())
    }

    fn library_usage(&self) -> AppResult<Vec<DocumentBytes>> {
        let listed = self.read_all_metas()?.into_iter().filter(|(_, meta)| meta.removed_at.is_none());
        listed.map(|(id, _)| Ok(DocumentBytes { bytes: self.document_bytes(&id)?, document_id: id })).collect()
    }
}

impl DocumentWriter for FsDocumentRepository {
    /// Writes the PDF (once: its bytes never change for an id), then the document, then `meta.json`.
    fn save(&self, document: &Document, original_pdf: &[u8], meta: &DocumentMeta) -> AppResult<()> {
        let directory = self.document_dir(&document.id);
        fs::create_dir_all(&directory).map_err(|error| storage_failed("creating", &directory, error))?;

        let pdf_path = directory.join(ORIGINAL_PDF_FILE);
        if !pdf_path.is_file() {
            write_atomically(&pdf_path, original_pdf).map_err(|error| storage_failed("writing", &pdf_path, error))?;
        }
        write_json(&directory.join(DOCUMENT_FILE), document)?;

        let _guard = self.lock_meta();
        let (meta, removed_at) = match self.read_meta(&document.id)? {
            Some(saved) => (meta.clone().merged_with_saved(&saved.meta(), document.sentence_count()), saved.removed_at),
            None => (meta.clone(), None),
        };
        write_json(&directory.join(META_FILE), &StoredMeta::new(document, &meta, removed_at))
    }

    fn save_position(&self, id: &DocumentId, position: ReadingPosition) -> AppResult<()> {
        self.update_meta(id, |meta| meta.last_sentence_id = Some(position.sentence_id))
    }

    fn mark_opened(&self, id: &DocumentId, opened_at: Timestamp) -> AppResult<()> {
        self.update_meta(id, |meta| meta.last_opened_at = opened_at)
    }

    fn set_removed(&self, id: &DocumentId, removed_at: Option<Timestamp>) -> AppResult<()> {
        self.update_meta(id, |meta| meta.removed_at = removed_at)
    }

    fn delete(&self, id: &DocumentId) -> AppResult<()> {
        let directory = self.document_dir(id);
        let meta_path = directory.join(META_FILE);
        let _guard = self.lock_meta();
        match fs::remove_file(&meta_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(storage_failed("deleting", &meta_path, error)),
        }
        match fs::remove_dir_all(&directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_failed("deleting", &directory, error)),
        }
    }
}

fn read_optional(path: &Path) -> AppResult<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_failed("reading", path, error)),
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<Option<T>> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|error| storage_failed("parsing", path, error))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| storage_failed("serializing", path, error))?;
    write_atomically(path, &bytes).map_err(|error| storage_failed("writing", path, error))
}

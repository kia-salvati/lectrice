use std::error::Error as StdError;
use std::fmt;

/// What the transport layer needs to turn an [`ErrorName`] into a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorSpec {
    pub code: u32,
    pub status: u16,
    pub operational: bool,
    pub description: &'static str,
}

macro_rules! error_registry {
    ($($name:ident => ($code:literal, $status:literal, $operational:literal, $description:literal)),+ $(,)?) => {
        /// Every error the app can raise. Services raise by name; only the transport layer reads [`ErrorName::spec`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ErrorName { $($name),+ }

        impl ErrorName {
            pub const ALL: &'static [ErrorName] = &[$(ErrorName::$name),+];

            pub const fn as_str(self) -> &'static str {
                match self { $(ErrorName::$name => stringify!($name)),+ }
            }

            pub const fn spec(self) -> ErrorSpec {
                match self {
                    $(ErrorName::$name => ErrorSpec { code: $code, status: $status, operational: $operational, description: $description }),+
                }
            }
        }
    };
}

error_registry! {
    RequestInvalid          => (1000, 400, true,  "This request isn't valid."),
    UploadMissingFile       => (1001, 400, true,  "No file was uploaded."),
    UploadTooLarge          => (1002, 413, true,  "This file is too large."),
    PdfInvalid              => (1003, 422, true,  "This file isn't a readable PDF."),
    PdfEncrypted            => (1004, 422, true,  "This PDF is password-protected."),
    PdfNoText               => (1005, 422, true,  "This PDF has no selectable text."),
    RouteNotFound           => (1006, 404, true,  "There is nothing at this address."),
    MethodNotAllowed        => (1007, 405, true,  "This address doesn't accept that method."),
    // 499 is the de facto "client closed request" status; the client is gone, so nobody receives it.
    RequestCancelled        => (1008, 499, true,  "The request was cancelled."),
    DocumentNotFound        => (1101, 404, true,  "Document not found."),
    SentenceNotFound        => (1102, 404, true,  "Sentence not found."),
    VoiceUnknown            => (2001, 400, true,  "Unknown voice."),
    SpeechEngineUnavailable => (2002, 503, true,  "The speech engine isn't available."),
    SpeechSynthesisFailed   => (2003, 500, false, "Speech generation failed."),
    ExportNotFound          => (3001, 404, true,  "Export not found."),
    ExportNotReady          => (3002, 409, true,  "The audio file isn't ready yet."),
    ExportFormatUnsupported => (3003, 400, true,  "That audio format isn't supported."),
    ExporterUnavailable     => (3004, 503, true,  "Audio export isn't available."),
    ModelMissing            => (4001, 503, true,  "The voice model isn't installed."),
    ModelDownloadFailed     => (4002, 502, true,  "The voice model couldn't be downloaded."),
    ModelChecksumMismatch   => (4003, 502, true,  "The downloaded voice model is damaged."),
    StorageFull             => (4004, 507, true,  "There isn't enough free space."),
    ExportPaused            => (4005, 409, true,  "Export paused; it will continue later."),
    StorageFailed           => (9001, 500, false, "Something went wrong saving data."),
    Internal                => (9999, 500, false, "Something went wrong."),
}

impl fmt::Display for ErrorName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

type BoxedSource = Box<dyn StdError + Send + Sync + 'static>;

/// A failure raised by one or more registry names, with an internal detail and cause chain for the logs.
/// The detail and the source are never sent to clients and must not contain book content.
#[derive(Debug)]
pub struct AppError {
    names: Vec<ErrorName>,
    detail: Option<String>,
    source: Option<BoxedSource>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(name: ErrorName) -> Self {
        Self { names: vec![name], detail: None, source: None }
    }

    /// Raises another name alongside the first; the first stays the primary one.
    pub fn and(mut self, name: ErrorName) -> Self {
        if !self.names.contains(&name) {
            self.names.push(name);
        }
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn primary(&self) -> ErrorName {
        self.names[0]
    }

    pub fn names(&self) -> &[ErrorName] {
        &self.names
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub fn is(&self, name: ErrorName) -> bool {
        self.names.contains(&name)
    }
}

impl From<ErrorName> for AppError {
    fn from(name: ErrorName) -> Self {
        Self::new(name)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.names.iter().map(|name| name.as_str()).collect();
        write!(f, "{}", names.join(", "))?;
        match &self.detail {
            Some(detail) => write!(f, ": {detail}"),
            None => Ok(()),
        }
    }
}

impl StdError for AppError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_deref().map(|source| source as &(dyn StdError + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Spec §8, plus `RequestInvalid` (B4), `RouteNotFound`, `MethodNotAllowed`, `RequestCancelled` (B-fixes)
    /// and the five device-neutral 4xxx names (Android spec C4).
    const SPEC_ERROR_COUNT: usize = 25;

    #[test]
    fn every_name_has_a_unique_code() {
        let codes: HashSet<u32> = ErrorName::ALL.iter().map(|name| name.spec().code).collect();
        assert_eq!(codes.len(), ErrorName::ALL.len());
    }

    #[test]
    fn every_name_is_unique_and_described() {
        let names: HashSet<&str> = ErrorName::ALL.iter().map(|name| name.as_str()).collect();
        assert_eq!(names.len(), SPEC_ERROR_COUNT);
        for name in ErrorName::ALL {
            let spec = name.spec();
            assert!(!spec.description.trim().is_empty(), "{name} has no description");
            assert!((400..600).contains(&spec.status), "{name} has a non-error status");
        }
    }

    #[test]
    fn first_raised_name_is_primary_and_duplicates_are_ignored() {
        let error = AppError::new(ErrorName::PdfInvalid).and(ErrorName::PdfEncrypted).and(ErrorName::PdfInvalid);
        assert_eq!(error.primary(), ErrorName::PdfInvalid);
        assert_eq!(error.names(), &[ErrorName::PdfInvalid, ErrorName::PdfEncrypted]);
        assert!(error.is(ErrorName::PdfEncrypted));
    }

    #[test]
    fn keeps_the_source_chain() {
        let io = std::io::Error::other("disk full");
        let error = AppError::new(ErrorName::StorageFailed).with_detail("writing document.json").with_source(io);
        assert_eq!(error.to_string(), "StorageFailed: writing document.json");
        assert_eq!(StdError::source(&error).map(|source| source.to_string()), Some("disk full".to_string()));
    }
}

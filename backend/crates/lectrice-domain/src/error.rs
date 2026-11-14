use std::error::Error as StdError;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|candidate| candidate.as_str() == name)
            }
        }
    };
}

/// Stored by name (in export records), so codes can be renumbered without breaking saved files.
impl Serialize for ErrorName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A name a newer version saved and this one doesn't know reads as `Internal`, so the record still loads.
impl<'de> Deserialize<'de> for ErrorName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(Self::from_name(&name).unwrap_or(Self::Internal))
    }
}

// One entry per line, `Name => (code, status, operational, "description"),`: the Android and web mirrors of these names
// parse this block in their tests, so keep the layout (comment lines in between are fine).
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
    RequestForbidden        => (1009, 403, true,  "This request didn't come from the Lectrice page."),
    PdfTooComplex           => (1010, 422, true,  "This PDF is too long or too complex to open."),
    DocumentNotFound        => (1101, 404, true,  "Document not found."),
    SentenceNotFound        => (1102, 404, true,  "Sentence not found."),
    VoiceUnknown            => (2001, 400, true,  "Unknown voice."),
    SpeechEngineUnavailable => (2002, 503, true,  "The speech engine isn't available."),
    SpeechSynthesisFailed   => (2003, 500, false, "Speech generation failed."),
    ExportNotFound          => (3001, 404, true,  "Export not found."),
    ExportNotReady          => (3002, 409, true,  "The audio file isn't ready yet."),
    ExportFormatUnsupported => (3003, 400, true,  "That audio format isn't supported."),
    ExporterUnavailable     => (3004, 503, true,  "Audio export isn't available."),
    AudiobookNotLectrice    => (3101, 422, true,  "This audiobook wasn't made by Lectrice."),
    AudiobookOtherBook      => (3102, 409, true,  "This audiobook belongs to another book."),
    AudiobookUnreadable     => (3103, 422, true,  "This file isn't an audiobook Lectrice can play."),
    ModelMissing            => (4001, 503, true,  "The voice model isn't installed."),
    ModelDownloadFailed     => (4002, 502, true,  "The voice model couldn't be downloaded."),
    ModelChecksumMismatch   => (4003, 502, true,  "The downloaded voice model is damaged."),
    StorageFull             => (4004, 507, true,  "There isn't enough free space."),
    ExportPaused            => (4005, 409, true,  "Export paused; it will continue later."),
    FileUnreadable          => (4006, 422, true,  "This file couldn't be opened."),
    HelperNotPaired         => (5001, 409, true,  "This phone isn't paired with the computer any more."),
    HelperPairingFailed     => (5002, 422, true,  "Pairing with the computer didn't work."),
    HelperUnreachable       => (5003, 503, true,  "The computer isn't reachable."),
    HelperModelMissing      => (5004, 409, true,  "The computer doesn't have this voice model."),
    HelperIncompatible      => (5005, 409, true,  "Lectrice on the phone and on the computer don't match; update the older one."),
    HelperBusy              => (5006, 429, true,  "The computer is busy; the phone makes this sentence itself."),
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
    /// the five device-neutral 4xxx names (Android spec C4), `FileUnreadable` (C26), the three audiobook names
    /// the six helper names 5001–5006, and `RequestForbidden`
    /// and `PdfTooComplex`.
    const SPEC_ERROR_COUNT: usize = 37;

    /// The mirrors in the other apps read the registry from this file's text, one entry per line.
    #[test]
    fn the_registry_block_keeps_one_entry_per_line() {
        let entry = regex::Regex::new(r#"^\s+([A-Za-z]+)\s+=> \((\d+), (\d{3}), (true|false),\s+"[^"]*"\),$"#).unwrap();
        let source = include_str!("error.rs");
        let block = source.split("\nerror_registry! {\n").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        let parsed: Vec<(String, u32)> = block
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| entry.captures(line).unwrap_or_else(|| panic!("not one entry: {line:?}")))
            .map(|captures| (captures[1].to_string(), captures[2].parse().unwrap()))
            .collect();
        let registry: Vec<(String, u32)> = ErrorName::ALL.iter().map(|name| (name.as_str().to_string(), name.spec().code)).collect();
        assert_eq!(parsed, registry);
    }

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
    fn file_unreadable_is_registered() {
        let spec = ErrorName::FileUnreadable.spec();
        assert_eq!((spec.code, spec.status, spec.operational), (4006, 422, true));
    }

    #[test]
    fn the_helper_names_are_5001_to_5006() {
        let helper = [
            ErrorName::HelperNotPaired,
            ErrorName::HelperPairingFailed,
            ErrorName::HelperUnreachable,
            ErrorName::HelperModelMissing,
            ErrorName::HelperIncompatible,
            ErrorName::HelperBusy,
        ];
        let codes: Vec<u32> = helper.iter().map(|name| name.spec().code).collect();
        assert_eq!(codes, (5001..=5006).collect::<Vec<_>>());
    }

    #[test]
    fn names_are_stored_by_name_and_unknown_names_read_as_internal() {
        assert_eq!(serde_json::to_value(ErrorName::StorageFull).unwrap(), "StorageFull");
        assert_eq!(serde_json::from_value::<ErrorName>("StorageFull".into()).unwrap(), ErrorName::StorageFull);
        assert_eq!(serde_json::from_value::<ErrorName>("SomethingNew".into()).unwrap(), ErrorName::Internal);
    }

    #[test]
    fn keeps_the_source_chain() {
        let io = std::io::Error::other("disk full");
        let error = AppError::new(ErrorName::StorageFailed).with_detail("writing document.json").with_source(io);
        assert_eq!(error.to_string(), "StorageFailed: writing document.json");
        assert_eq!(StdError::source(&error).map(|source| source.to_string()), Some("disk full".to_string()));
    }
}

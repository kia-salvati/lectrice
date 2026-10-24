//! Dev command for reviewing the cleaner: `lectrice-api clean <pdf> --out <dir> [--pages 27-40]`.
//! Writes `<name>.document.json` (the saved format) and `<name>.txt` (a readable dump, optionally
//! only the blocks that start in a page range).

mod dump;

use std::error::Error;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use lectrice_app::documents::DocumentPipeline;
use lectrice_domain::cleaning::RuleBasedCleaner;
use lectrice_domain::{ContentHasher, DocumentCleaner, PdfTextSource};
use lectrice_infra::{PdfiumTextSource, SentencexSplitter, Sha256ContentHasher};

use crate::config::PdfConfig;

pub const COMMAND: &str = "clean";

const SERVICE: &str = "devClean";
const OUT_FLAG: &str = "--out";
const PAGES_FLAG: &str = "--pages";
const USAGE: &str = "usage: lectrice-api clean <file.pdf> --out <dir> [--pages 27-40]";
const JSON_SUFFIX: &str = ".document.json";
const DUMP_SUFFIX: &str = ".txt";

#[derive(Debug, PartialEq, Eq)]
struct CleanArgs {
    pdf: PathBuf,
    out: PathBuf,
    pages: Option<RangeInclusive<u32>>,
}

/// `args` are the command-line arguments after `clean`.
pub fn run(config: &PdfConfig, args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let args = parse(args)?;
    let bytes = std::fs::read(&args.pdf)?;
    let file_name = args.pdf.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let source = Arc::new(PdfiumTextSource::new(&config.pdfium_library)?);
    let cleaner = Arc::new(RuleBasedCleaner);

    let started = Instant::now();
    let content = source.read_runs(&bytes)?;
    let read_ms = started.elapsed().as_millis();
    let started = Instant::now();
    let cleaned = cleaner.clean(content)?;
    let clean_ms = started.elapsed().as_millis();
    let id = Sha256ContentHasher.document_id(&bytes)?;
    tracing::info!(service = SERVICE, doc_id = %id, pages = cleaned.page_count, blocks = cleaned.blocks.len(), read_ms, clean_ms, "PDF cleaned");

    let document = DocumentPipeline::new(source, cleaner, Arc::new(SentencexSplitter)).process(&id, &file_name, &bytes)?;

    std::fs::create_dir_all(&args.out)?;
    let stem = args.pdf.file_stem().map_or_else(|| "document".to_string(), |stem| stem.to_string_lossy().into_owned());
    write(&args.out, &stem, JSON_SUFFIX, &serde_json::to_string_pretty(&document)?)?;
    write(&args.out, &stem, DUMP_SUFFIX, &dump::render(&document, &cleaned, args.pages.as_ref()))?;
    tracing::info!(service = SERVICE, doc_id = %id, out_dir = %args.out.display(), "Cleaned document written");
    Ok(())
}

fn write(dir: &Path, stem: &str, suffix: &str, contents: &str) -> std::io::Result<()> {
    std::fs::write(dir.join(format!("{stem}{suffix}")), contents)
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<CleanArgs, String> {
    let mut args = args.into_iter();
    let pdf = args.next().filter(|first| !first.starts_with("--")).map(PathBuf::from).ok_or_else(|| USAGE.to_owned())?;
    let (mut out, mut pages) = (None, None);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value; {USAGE}"))?;
        match flag.as_str() {
            OUT_FLAG => out = Some(PathBuf::from(value)),
            PAGES_FLAG => pages = Some(parse_pages(&value)?),
            _ => return Err(format!("unknown argument {flag}; {USAGE}")),
        }
    }
    let out = out.ok_or_else(|| USAGE.to_owned())?;
    Ok(CleanArgs { pdf, out, pages })
}

fn parse_pages(value: &str) -> Result<RangeInclusive<u32>, String> {
    let invalid = || format!("--pages must look like 27-40; {USAGE}");
    let (first, last) = value.split_once('-').ok_or_else(invalid)?;
    let first: u32 = first.trim().parse().map_err(|_| invalid())?;
    let last: u32 = last.trim().parse().map_err(|_| invalid())?;
    if first > last {
        return Err(invalid());
    }
    Ok(first..=last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_pdf_out_and_pages() {
        let args = parse(strings(&["book.pdf", "--out", "dumps", "--pages", "27-40"])).unwrap();
        assert_eq!(args, CleanArgs { pdf: "book.pdf".into(), out: "dumps".into(), pages: Some(27..=40) });
        assert_eq!(parse(strings(&["book.pdf", "--out", "dumps"])).unwrap().pages, None);
    }

    #[test]
    fn rejects_missing_pdf_or_out_and_bad_pages() {
        assert!(parse(strings(&["--out", "dumps"])).is_err());
        assert!(parse(strings(&["book.pdf"])).is_err());
        assert!(parse(strings(&["book.pdf", "--out", "d", "--pages", "40-27"])).is_err());
        assert!(parse(strings(&["book.pdf", "--out", "d", "--pages", "x"])).is_err());
    }
}

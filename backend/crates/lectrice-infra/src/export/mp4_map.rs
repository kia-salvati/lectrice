//! The Lectrice map in an M4B (spec 23 §3.1–3.2): written as a top-level `uuid` box after the file is assembled (no
//! sample offset moves, players skip unknown boxes), and read back with the file's length, chapter marks and title
//! without touching the audio.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use lectrice_domain::{AppError, AppResult, AudiobookInfo, ChapterMark, ErrorName, LectriceMap};

/// ISO/IEC 14496-12 §4.2 extended type: ASCII "lectrice-map-v01".
pub const MAP_UUID: [u8; 16] = *b"lectrice-map-v01";
const HEADER: u64 = 8;
const LARGE_HEADER: u64 = 16;
const UUID_LEN: usize = 16;
/// A `moov` bigger than this is refused (a broken or hostile file; a 40-hour book's is a few MB).
const MAX_MOOV_BYTES: u64 = 64 * 1024 * 1024;
/// A map bigger than this is not read (the 200 000 sentences a map may hold take about 6 MB); the file then reads as a
/// foreign audiobook.
const MAX_MAP_BYTES: u64 = 8 * 1024 * 1024;
/// Top-level boxes walked before the file is refused (a fragmented 40-hour file has fewer).
const MAX_TOP_LEVEL_BOXES: u32 = 200_000;
/// The title tag is cut to this many bytes.
const MAX_TITLE_BYTES: usize = 1024;
const MILLIS_PER_SECOND: u64 = 1000;
/// `chpl` times are in 100 ns units.
const HUNDRED_NS_PER_MS: u64 = 10_000;
/// A `chpl` entry's start time, before its title's length byte.
const CHPL_START_BYTES: usize = 8;
const TITLE_ITEM: [u8; 4] = [0xa9, b'n', b'a', b'm'];
/// The `data` box's type + locale words before an iTunes item's value.
const ITEM_VALUE_OFFSET: usize = 8;

/// Appends the map to a finished M4B.
pub fn append_map(path: &Path, map: &LectriceMap) -> AppResult<()> {
    let payload = serde_json::to_vec(map).map_err(|error| AppError::new(ErrorName::Internal).with_source(error))?;
    let size = u32::try_from(HEADER as usize + UUID_LEN + payload.len()).map_err(|_| AppError::new(ErrorName::Internal).with_detail("the map is too large"))?;
    let mut file = OpenOptions::new().append(true).open(path).map_err(storage)?;
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&size.to_be_bytes());
    bytes.extend_from_slice(b"uuid");
    bytes.extend_from_slice(&MAP_UUID);
    bytes.extend_from_slice(&payload);
    file.write_all(&bytes).map_err(storage)?;
    file.sync_all().map_err(storage)
}

/// Reads what spec 23 §3.2 needs. The file is untrusted: every size is checked against the file's length and capped
/// before anything is allocated. `AudiobookUnreadable` when it isn't an MP4 with an AAC track; a map that doesn't parse
/// or can't be true is left out and the file reads as a foreign audiobook.
pub fn read_audiobook(path: &Path) -> AppResult<AudiobookInfo> {
    let mut file = File::open(path).map_err(|error| AppError::new(ErrorName::FileUnreadable).with_source(error))?;
    let length = file.metadata().map_err(storage)?.len();
    let mut info = AudiobookInfo::default();
    let mut has_audio = false;
    let mut boxes = 0u32;
    let mut offset = 0u64;
    // `box_header` proves `offset + size <= length`, so the sums below can't overflow.
    while length - offset >= HEADER {
        let (kind, size, header) = box_header(&mut file, offset, length)?;
        if boxes == 0 && kind != *b"ftyp" {
            return Err(unreadable("no ftyp box"));
        }
        boxes += 1;
        if boxes > MAX_TOP_LEVEL_BOXES {
            return Err(unreadable("too many boxes"));
        }
        let (body_at, body_len) = (offset + header, size - header);
        match &kind {
            b"moov" if !has_audio => {
                if body_len > MAX_MOOV_BYTES {
                    return Err(unreadable("the moov box is too large"));
                }
                let moov = read_body(&mut file, body_at, body_len)?;
                has_audio = read_moov(&moov, &mut info);
            }
            b"uuid" if info.map.is_none() && body_len > UUID_LEN as u64 => info.map = read_map(&mut file, body_at, body_len)?,
            _ => {}
        }
        offset += size;
    }
    if !has_audio {
        return Err(unreadable("no AAC audio track"));
    }
    Ok(info)
}

/// The map in a `uuid` box, when the box is ours. Only the 16-byte type is read from other `uuid` boxes.
fn read_map(file: &mut File, body_at: u64, body_len: u64) -> AppResult<Option<LectriceMap>> {
    let kind = read_body(file, body_at, UUID_LEN as u64)?;
    if kind != MAP_UUID {
        return Ok(None);
    }
    let payload_len = body_len - UUID_LEN as u64;
    if payload_len > MAX_MAP_BYTES {
        return Ok(None);
    }
    Ok(parse_map(&read_body(file, body_at + UUID_LEN as u64, payload_len)?))
}

/// A map of an unknown version, one that doesn't parse, or one whose values can't be true ([`LectriceMap::check`])
/// reads as no map: the file is then a foreign audiobook.
fn parse_map(payload: &[u8]) -> Option<LectriceMap> {
    serde_json::from_slice::<LectriceMap>(payload).ok().filter(|map| map.is_supported() && map.check().is_ok())
}

/// True when the movie has an AAC (`mp4a`) track. Fills the length, chapter marks and title.
fn read_moov(moov: &[u8], info: &mut AudiobookInfo) -> bool {
    for (kind, body) in children(moov) {
        match &kind {
            b"mvhd" => info.duration_ms = mvhd_duration_ms(body).unwrap_or(0),
            b"udta" => {
                for (kind, body) in children(body) {
                    match &kind {
                        b"chpl" => info.chapters = chpl(body),
                        b"meta" if body.len() > 4 => info.title = title(&body[4..]),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    moov.windows(4).any(|window| window == b"mp4a")
}

fn mvhd_duration_ms(body: &[u8]) -> Option<u64> {
    let version = *body.first()?;
    let (timescale, duration) =
        if version == 1 { (u64::from(be_u32(body, 20)?), be_u64(body, 24)?) } else { (u64::from(be_u32(body, 12)?), u64::from(be_u32(body, 16)?)) };
    if timescale == 0 {
        return None;
    }
    u64::try_from(u128::from(duration) * u128::from(MILLIS_PER_SECOND) / u128::from(timescale)).ok()
}

/// Nero chapters: version, flags, (a reserved word in version 1), a count, then (start in 100 ns, title length, title).
fn chpl(body: &[u8]) -> Vec<ChapterMark> {
    let mut chapters = Vec::new();
    let Some(&version) = body.first() else { return chapters };
    let mut at = if version == 1 { 8 } else { 4 };
    let Some(&count) = body.get(at) else { return chapters };
    at += 1;
    for _ in 0..count {
        let (Some(start), Some(&len)) = (be_u64(body, at), body.get(at + CHPL_START_BYTES)) else { break };
        let from = at + CHPL_START_BYTES + 1;
        let Some(title) = body.get(from..from + usize::from(len)) else { break };
        chapters.push(ChapterMark { title: String::from_utf8_lossy(title).into_owned(), start_ms: start / HUNDRED_NS_PER_MS });
        at = from + usize::from(len);
    }
    chapters
}

/// `ilst/©nam/data`'s text, from the body of `meta` after its version word.
fn title(meta: &[u8]) -> Option<String> {
    let ilst = children(meta).find(|(kind, _)| kind == b"ilst")?.1;
    let item = children(ilst).find(|(kind, _)| *kind == TITLE_ITEM)?.1;
    let data = children(item).find(|(kind, _)| kind == b"data")?.1;
    let value = data.get(ITEM_VALUE_OFFSET..)?;
    let text = String::from_utf8_lossy(&value[..value.len().min(MAX_TITLE_BYTES)]);
    Some(text.chars().filter(|c| !c.is_control() && *c != char::REPLACEMENT_CHARACTER).collect::<String>().trim().to_string()).filter(|title| !title.is_empty())
}

/// The boxes inside a box body (32-bit sizes only, as inside `moov`); stops at the first malformed one.
fn children(body: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let size = usize::try_from(be_u32(body, at)?).ok()?;
        let kind: [u8; 4] = body.get(at + 4..at + 8)?.try_into().ok()?;
        let end = at.checked_add(size).filter(|end| size >= HEADER as usize && *end <= body.len())?;
        let child = &body[at + HEADER as usize..end];
        at = end;
        Some((kind, child))
    })
}

/// (type, whole size, header size) of the box at `offset`.
fn box_header(file: &mut File, offset: u64, length: u64) -> AppResult<([u8; 4], u64, u64)> {
    let mut header = [0u8; HEADER as usize];
    file.seek(SeekFrom::Start(offset)).map_err(storage)?;
    file.read_exact(&mut header).map_err(|error| AppError::new(ErrorName::AudiobookUnreadable).with_source(error))?;
    let kind: [u8; 4] = header[4..].try_into().map_err(|_| unreadable("box header"))?;
    let (size, header_len) = match u32::from_be_bytes(header[..4].try_into().map_err(|_| unreadable("box header"))?) {
        0 => (length - offset, HEADER),
        1 => {
            let mut large = [0u8; 8];
            file.read_exact(&mut large).map_err(|error| AppError::new(ErrorName::AudiobookUnreadable).with_source(error))?;
            (u64::from_be_bytes(large), LARGE_HEADER)
        }
        size => (u64::from(size), HEADER),
    };
    if size < header_len || offset.checked_add(size).is_none_or(|end| end > length) {
        return Err(unreadable("a box runs past the end of the file"));
    }
    Ok((kind, size, header_len))
}

/// `len` bytes at `offset`. Callers cap `len` first ([MAX_MOOV_BYTES], [MAX_MAP_BYTES]).
fn read_body(file: &mut File, offset: u64, len: u64) -> AppResult<Vec<u8>> {
    let mut body = vec![0u8; usize::try_from(len).map_err(|_| unreadable("a box is too large"))?];
    file.seek(SeekFrom::Start(offset)).map_err(storage)?;
    file.read_exact(&mut body).map_err(|error| AppError::new(ErrorName::AudiobookUnreadable).with_source(error))?;
    Ok(body)
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn be_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn unreadable(detail: &str) -> AppError {
    AppError::new(ErrorName::AudiobookUnreadable).with_detail(detail.to_string())
}

fn storage(error: std::io::Error) -> AppError {
    AppError::new(ErrorName::StorageFailed).with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lectrice_domain::{DocumentId, SentenceId, SentenceMark, audiobook::MAP_FORMAT};

    fn mp4_box(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut out = u32::try_from(body.len() + 8).unwrap().to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    /// An invented, minimal M4B: ftyp, an mdat of zeros, and a moov with mvhd (2.5 s), an mp4a mention, chpl and a title.
    fn tiny_m4b() -> Vec<u8> {
        let mut mvhd = vec![0u8; 100];
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&2500u32.to_be_bytes());
        let mut chpl = vec![1, 0, 0, 0, 0, 0, 0, 0, 2];
        for (start_ms, title) in [(0u64, "1 The Harbour"), (1200, "2 The Lamp")] {
            chpl.extend_from_slice(&(start_ms * HUNDRED_NS_PER_MS).to_be_bytes());
            chpl.push(u8::try_from(title.len()).unwrap());
            chpl.extend_from_slice(title.as_bytes());
        }
        let data = mp4_box(b"data", &[[0, 0, 0, 1, 0, 0, 0, 0].as_slice(), b"The Long Book"].concat());
        let ilst = mp4_box(b"ilst", &mp4_box(&TITLE_ITEM, &data));
        let meta = mp4_box(b"meta", &[[0u8; 4].as_slice(), &ilst].concat());
        let udta = mp4_box(b"udta", &[mp4_box(b"chpl", &chpl), meta].concat());
        let trak = mp4_box(b"trak", &mp4_box(b"stsd", b"....mp4a...."));
        let moov = mp4_box(b"moov", &[mp4_box(b"mvhd", &mvhd), trak, udta].concat());
        [mp4_box(b"ftyp", b"M4A mp42"), mp4_box(b"mdat", &[0u8; 64]), moov].concat()
    }

    fn map() -> LectriceMap {
        LectriceMap {
            format: MAP_FORMAT.into(),
            version: 1,
            document_id: DocumentId::parse(&"ab".repeat(32)).unwrap(),
            cleaner_version: 4,
            engine_id: "kokoro-test".into(),
            voice_id: "af_heart".into(),
            speed_percent: 100,
            read_skipped: false,
            range: None,
            sample_rate: 24_000,
            duration_ms: 2_500,
            sentences: vec![SentenceMark(SentenceId(0), "01234567".into(), 0), SentenceMark(SentenceId(1), "89abcdef".into(), 1_200)],
        }
    }

    #[test]
    fn a_foreign_file_reads_its_length_chapters_and_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        std::fs::write(&path, tiny_m4b()).unwrap();
        let info = read_audiobook(&path).unwrap();
        assert_eq!(info.duration_ms, 2_500);
        assert_eq!(
            info.chapters,
            vec![ChapterMark { title: "1 The Harbour".into(), start_ms: 0 }, ChapterMark { title: "2 The Lamp".into(), start_ms: 1_200 }]
        );
        assert_eq!(info.title.as_deref(), Some("The Long Book"));
        assert!(info.map.is_none());
    }

    #[test]
    fn an_appended_map_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        std::fs::write(&path, tiny_m4b()).unwrap();
        append_map(&path, &map()).unwrap();
        assert_eq!(read_audiobook(&path).unwrap().map, Some(map()));
    }

    #[test]
    fn a_damaged_map_reads_as_foreign() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        std::fs::write(&path, [tiny_m4b(), mp4_box(b"uuid", &[MAP_UUID.as_slice(), b"{not json"].concat())].concat()).unwrap();
        let info = read_audiobook(&path).unwrap();
        assert!(info.map.is_none());
        assert_eq!(info.chapters.len(), 2);
    }

    fn read_bytes(bytes: &[u8]) -> AppResult<AudiobookInfo> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        std::fs::write(&path, bytes).unwrap();
        read_audiobook(&path)
    }

    fn map_box(map: &LectriceMap) -> Vec<u8> {
        mp4_box(b"uuid", &[MAP_UUID.as_slice(), &serde_json::to_vec(map).unwrap()].concat())
    }

    #[test]
    fn a_map_with_times_that_go_back_reads_as_foreign() {
        let mut reversed = map();
        reversed.sentences.reverse();
        let info = read_bytes(&[tiny_m4b(), map_box(&reversed)].concat()).unwrap();
        assert!(info.map.is_none());
        let mut stopped = map();
        stopped.speed_percent = 0;
        assert!(read_bytes(&[tiny_m4b(), map_box(&stopped)].concat()).unwrap().map.is_none());
    }

    #[test]
    fn a_map_over_the_size_cap_is_not_read() {
        let mut large = map();
        large.engine_id = "e".repeat(usize::try_from(MAX_MAP_BYTES).unwrap());
        assert!(read_bytes(&[tiny_m4b(), map_box(&large)].concat()).unwrap().map.is_none());
    }

    #[test]
    fn only_the_first_map_counts_and_other_uuid_boxes_are_skipped() {
        let mut second = map();
        second.voice_id = "af_bella".into();
        let other = mp4_box(b"uuid", &[b"someone-elses-box".as_slice(), &[0u8; 64]].concat());
        let info = read_bytes(&[tiny_m4b(), other, map_box(&map()), map_box(&second)].concat()).unwrap();
        assert_eq!(info.map, Some(map()));
    }

    #[test]
    fn box_sizes_that_overflow_or_leave_the_file_are_refused() {
        let large_size = |size: u64| [1u32.to_be_bytes().as_slice(), b"free", &size.to_be_bytes()].concat();
        for size in [u64::MAX, u64::MAX - 7, 1 << 63, 15, 0] {
            let error = read_bytes(&[mp4_box(b"ftyp", b"M4A mp42"), large_size(size), tiny_m4b()].concat()).unwrap_err();
            assert_eq!(error.primary(), ErrorName::AudiobookUnreadable, "size {size}");
        }
        let short = [mp4_box(b"ftyp", b"M4A mp42"), 4u32.to_be_bytes().to_vec(), b"free".to_vec(), tiny_m4b()].concat();
        assert_eq!(read_bytes(&short).unwrap_err().primary(), ErrorName::AudiobookUnreadable);
    }

    #[test]
    fn a_huge_declared_moov_is_refused_without_reading_it() {
        // A sparse file: the header says 100 MB, nothing is allocated for it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.m4b");
        let size = MAX_MOOV_BYTES + 100 * 1024 * 1024;
        let header = [u32::try_from(size).unwrap().to_be_bytes().as_slice(), b"moov"].concat();
        std::fs::write(&path, [mp4_box(b"ftyp", b"M4A mp42"), header].concat()).unwrap();
        OpenOptions::new().write(true).open(&path).unwrap().set_len(16 + size).unwrap();
        assert_eq!(read_audiobook(&path).unwrap_err().primary(), ErrorName::AudiobookUnreadable);
    }

    #[test]
    fn mvhd_with_extreme_numbers_does_not_overflow() {
        let mut body = vec![0u8; 112];
        body[0] = 1;
        body[20..24].copy_from_slice(&1u32.to_be_bytes());
        body[24..32].copy_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(mvhd_duration_ms(&body), None);
        body[20..24].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(mvhd_duration_ms(&body), None);
        assert_eq!(mvhd_duration_ms(&[]), None);
    }

    #[test]
    fn the_title_is_cut_and_cleaned() {
        let long = format!("A\u{0}Title\n{}", "x".repeat(4 * MAX_TITLE_BYTES));
        let data = mp4_box(b"data", &[[0, 0, 0, 1, 0, 0, 0, 0].as_slice(), long.as_bytes()].concat());
        let ilst = mp4_box(b"ilst", &mp4_box(&TITLE_ITEM, &data));
        let found = title(&ilst).unwrap();
        assert!(found.starts_with("ATitlex"));
        assert!(found.len() <= MAX_TITLE_BYTES);
    }

    #[test]
    fn not_an_mp4_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.m4b");
        std::fs::write(&path, b"%PDF-1.7 hello").unwrap();
        assert_eq!(read_audiobook(&path).unwrap_err().primary(), ErrorName::AudiobookUnreadable);
    }
}

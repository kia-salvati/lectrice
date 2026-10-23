//! The ffmpeg metadata file that carries an M4B's tags and chapter marks.

use lectrice_domain::ChapterMark;

const HEADER: &str = ";FFMETADATA1";
const TIMEBASE: &str = "1/1000";

pub(crate) struct Tags<'a> {
    pub title: &'a str,
    pub artist: &'a str,
}

/// Chapters run from their start to the next one's start (the last to `total_ms`). Audio before the
/// first mark becomes a chapter named after the book, and empty chapters (two marks at one spot) are dropped.
pub(crate) fn ffmetadata(tags: &Tags<'_>, marks: &[ChapterMark], total_ms: u64) -> String {
    let mut text = format!("{HEADER}\ntitle={}\nartist={}\nalbum={}\ngenre=Audiobook\n", escape(tags.title), escape(tags.artist), escape(tags.title));
    for (title, start_ms, end_ms) in chapter_spans(tags.title, marks, total_ms) {
        text.push_str(&format!("\n[CHAPTER]\nTIMEBASE={TIMEBASE}\nSTART={start_ms}\nEND={end_ms}\ntitle={}\n", escape(title)));
    }
    text
}

fn chapter_spans<'a>(book_title: &'a str, marks: &'a [ChapterMark], total_ms: u64) -> Vec<(&'a str, u64, u64)> {
    let mut starts: Vec<(&str, u64)> = Vec::new();
    if marks.first().is_none_or(|first| first.start_ms > 0) {
        starts.push((book_title, 0));
    }
    starts.extend(marks.iter().map(|mark| (mark.title.as_str(), mark.start_ms.min(total_ms))));
    let mut spans = Vec::new();
    for (index, &(title, start)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(total_ms, |&(_, next)| next);
        if end > start {
            spans.push((title, start, end));
        }
    }
    spans
}

/// ffmetadata treats `=`, `;`, `#`, `\` and line breaks as syntax.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '=' | ';' | '#' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '\n' | '\r' => escaped.push(' '),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(title: &str, start_ms: u64) -> ChapterMark {
        ChapterMark { title: title.into(), start_ms }
    }

    fn chapters(text: &str) -> Vec<String> {
        text.split("[CHAPTER]\n").skip(1).map(|chapter| chapter.lines().skip(1).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("|")).collect()
    }

    #[test]
    fn chapters_run_to_the_next_mark_and_the_end() {
        let tags = Tags { title: "Book", artist: "Heart" };
        let text = ffmetadata(&tags, &[mark("One", 0), mark("Two", 2500)], 4000);

        assert!(text.starts_with(";FFMETADATA1\ntitle=Book\nartist=Heart\n"));
        assert_eq!(chapters(&text), ["START=0|END=2500|title=One", "START=2500|END=4000|title=Two"]);
    }

    #[test]
    fn audio_before_the_first_mark_is_named_after_the_book_and_empty_chapters_are_dropped() {
        let tags = Tags { title: "Book", artist: "Heart" };
        let text = ffmetadata(&tags, &[mark("Part", 1000), mark("Chapter", 1000)], 3000);

        assert_eq!(chapters(&text), ["START=0|END=1000|title=Book", "START=1000|END=3000|title=Chapter"]);
    }

    #[test]
    fn syntax_characters_are_escaped() {
        let tags = Tags { title: "a=b; #c\\d\ne", artist: "x" };
        assert!(ffmetadata(&tags, &[], 1).contains("title=a\\=b\\; \\#c\\\\d e\n"));
    }
}

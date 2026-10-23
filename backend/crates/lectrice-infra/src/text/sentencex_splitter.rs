use std::ops::Range;

use lectrice_domain::SentenceSplitter;

const LANGUAGE: &str = "en";
/// Abbreviations that sentencex may split after; a sentence ending in one of them is joined to the next.
/// Compared case-insensitively against the sentence's last word.
const EXTRA_ABBREVIATIONS: [&str; 14] = ["e.g.", "i.e.", "dr.", "fig.", "figs.", "al.", "eq.", "eqs.", "u.s.", "u.k.", "vs.", "cf.", "approx.", "ch."];

/// English sentence splitting with sentencex plus the cases technical books need.
#[derive(Debug, Default, Clone, Copy)]
pub struct SentencexSplitter;

impl SentenceSplitter for SentencexSplitter {
    fn split(&self, text: &str) -> Vec<Range<usize>> {
        let raw = sentencex::get_sentence_boundaries(LANGUAGE, text).into_iter().map(|boundary| boundary.start_byte..boundary.end_byte);
        let mut sentences: Vec<Range<usize>> = Vec::new();
        for range in raw {
            let Some(range) = trim(text, range) else { continue };
            match sentences.last_mut() {
                Some(previous) if should_join(text, previous, &range) => previous.end = range.end,
                _ => sentences.push(range),
            }
        }
        sentences
    }
}

fn trim(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let slice = text.get(range.clone())?;
    let start = range.start + (slice.len() - slice.trim_start().len());
    let end = range.start + slice.trim_end().len();
    (start < end).then_some(start..end)
}

fn should_join(text: &str, previous: &Range<usize>, next: &Range<usize>) -> bool {
    let before = &text[previous.clone()];
    let after = &text[next.clone()];
    let last_word = before.rsplit(char::is_whitespace).next().unwrap_or_default().to_lowercase();
    let ends_with_abbreviation = EXTRA_ABBREVIATIONS.contains(&last_word.as_str());
    let splits_a_decimal = before.ends_with('.')
        && before[..before.len() - 1].ends_with(|character: char| character.is_ascii_digit())
        && next.start == previous.end
        && after.starts_with(|character: char| character.is_ascii_digit());
    ends_with_abbreviation || splits_a_decimal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentences(text: &str) -> Vec<&str> {
        SentencexSplitter.split(text).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn plain_sentences_split_and_are_trimmed() {
        assert_eq!(sentences("Hello world.  This is a test. "), ["Hello world.", "This is a test."]);
        assert_eq!(sentences("Is it? Yes! Done."), ["Is it?", "Yes!", "Done."]);
    }

    #[test]
    fn abbreviations_do_not_end_sentences() {
        assert_eq!(sentences("Use a cache, e.g. Redis, for this. Then go."), ["Use a cache, e.g. Redis, for this.", "Then go."]);
        assert_eq!(sentences("It works, i.e. It runs. Next."), ["It works, i.e. It runs.", "Next."]);
        assert_eq!(sentences("Ask Dr. Smith about it. He knows."), ["Ask Dr. Smith about it.", "He knows."]);
        assert_eq!(sentences("See Fig. 3 for details. It helps."), ["See Fig. 3 for details.", "It helps."]);
        assert_eq!(sentences("As Gamma et al. Showed in 1994. True."), ["As Gamma et al. Showed in 1994.", "True."]);
        assert_eq!(sentences("Apply Eq. 2 here. Done."), ["Apply Eq. 2 here.", "Done."]);
        assert_eq!(sentences("Made in the U.S. Army labs. Yes."), ["Made in the U.S. Army labs.", "Yes."]);
    }

    #[test]
    fn decimals_and_quotes_stay_whole() {
        assert_eq!(sentences("Pi is 3.14 roughly. Next."), ["Pi is 3.14 roughly.", "Next."]);
        assert_eq!(sentences("Version 1.29 shipped. Then 2.0 came."), ["Version 1.29 shipped.", "Then 2.0 came."]);
        assert_eq!(sentences("He said “Stop.” Then he left."), ["He said “Stop.”", "Then he left."]);
    }

    #[test]
    fn ranges_index_the_original_text() {
        let text = "  One. Two.";
        assert_eq!(SentencexSplitter.split(text), [2..6, 7..11]);
        assert!(SentencexSplitter.split("   ").is_empty());
    }
}

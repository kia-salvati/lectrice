//! Rule 17: line-end hyphens. U+2010 (O'Reilly) is always a syllable break. A plain hyphen, which
//! PDFium marks at a line end, is removed only when the book itself spells the joined word without
//! one somewhere and never writes the hyphenated form mid-line.

use std::collections::HashSet;

use super::lines::Line;

pub const UNICODE_HYPHEN: char = '\u{2010}';
const ASCII_HYPHEN: char = '-';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// Join with a space (no hyphen at the line end).
    Space,
    /// Remove the line-end hyphen and join without a space (`Unfortu-` + `nately`).
    RemoveHyphen,
    /// Keep the hyphen and join without a space (`first-` + `choice`).
    KeepHyphen,
}

/// The words of a book as it spells them inside lines.
#[derive(Debug, Default)]
pub struct Vocabulary {
    words: HashSet<String>,
    hyphenated: HashSet<String>,
}

impl Vocabulary {
    pub fn from_lines<'a>(lines: impl Iterator<Item = &'a Line>) -> Self {
        let mut vocabulary = Self::default();
        for line in lines {
            let tokens: Vec<&str> = line.text.split_whitespace().collect();
            let last = tokens.len().saturating_sub(1);
            for (position, token) in tokens.iter().enumerate() {
                let breaks_here = position == last && token.ends_with([ASCII_HYPHEN, UNICODE_HYPHEN]);
                if !breaks_here {
                    vocabulary.add(token);
                }
            }
        }
        vocabulary
    }

    fn add(&mut self, token: &str) {
        let word = token.trim_matches(|character: char| !character.is_alphanumeric()).to_lowercase();
        if word.is_empty() {
            return;
        }
        if word.contains(ASCII_HYPHEN) {
            self.hyphenated.insert(word);
        } else {
            self.words.insert(word);
        }
    }

    /// How `upper` (ending in a hyphen) joins `lower`, the next line of the same block.
    pub fn join(&self, upper: &str, upper_marked: bool, lower: &str) -> LineJoin {
        if upper.ends_with(UNICODE_HYPHEN) {
            return LineJoin::RemoveHyphen;
        }
        let Some(before_hyphen) = upper.strip_suffix(ASCII_HYPHEN) else { return LineJoin::Space };
        let left = trailing_letters(before_hyphen);
        let right = leading_letters(lower);
        if left.is_empty() || right.is_empty() {
            return if upper_marked { LineJoin::KeepHyphen } else { LineJoin::Space };
        }
        if right.starts_with(char::is_uppercase) {
            return LineJoin::KeepHyphen;
        }
        let joined = format!("{left}{right}").to_lowercase();
        let hyphenated = format!("{left}-{right}").to_lowercase();
        if self.words.contains(&joined) && !self.hyphenated.contains(&hyphenated) { LineJoin::RemoveHyphen } else { LineJoin::KeepHyphen }
    }
}

fn trailing_letters(text: &str) -> &str {
    let start = text.char_indices().rev().take_while(|(_, character)| character.is_alphabetic()).last().map_or(text.len(), |(index, _)| index);
    &text[start..]
}

fn leading_letters(text: &str) -> &str {
    let end = text.char_indices().find(|(_, character)| !character.is_alphabetic()).map_or(text.len(), |(index, _)| index);
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary(words: &[&str]) -> Vocabulary {
        let mut vocabulary = Vocabulary::default();
        for word in words {
            vocabulary.add(word);
        }
        vocabulary
    }

    #[test]
    fn unicode_hyphen_is_always_a_syllable_break() {
        assert_eq!(Vocabulary::default().join("a quiet net‐", false, "work zone"), LineJoin::RemoveHyphen);
    }

    #[test]
    fn known_words_join_and_unknown_compounds_keep_the_hyphen() {
        let book = vocabulary(&["fortunately,", "windows", "baskets", "red-box", "first"]);
        assert_eq!(book.join("Fortu-", true, "nately, a ten-year-old map"), LineJoin::RemoveHyphen);
        assert_eq!(book.join("is the first-", true, "rate pick"), LineJoin::KeepHyphen);
        assert_eq!(book.join("the red-", true, "box Box"), LineJoin::KeepHyphen);
        assert_eq!(book.join("Smith-", true, "Jones"), LineJoin::KeepHyphen);
    }

    #[test]
    fn a_word_also_seen_hyphenated_keeps_its_hyphen() {
        let book = vocabulary(&["preexisting", "pre-existing"]);
        assert_eq!(book.join("Some pre-", true, "existing tools"), LineJoin::KeepHyphen);
    }

    #[test]
    fn lines_without_a_hyphen_join_with_a_space() {
        assert_eq!(Vocabulary::default().join("and much", false, "lighter."), LineJoin::Space);
        assert_eq!(Vocabulary::default().join("a dash -", false, "here"), LineJoin::Space);
    }

    #[test]
    fn the_last_token_of_a_hyphen_line_is_not_learned() {
        let lines = [crate::cleaning::lines::tests::line_with_text("it was fortu-")];
        let book = Vocabulary::from_lines(lines.iter());
        assert!(book.words.contains("it") && !book.words.contains("fortu"));
    }
}

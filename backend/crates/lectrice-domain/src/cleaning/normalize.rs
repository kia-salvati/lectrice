//! Rule 18: Unicode clean-up of run text (NFKC turns LinkedIn's styled letters and ligatures into plain text).

use unicode_normalization::UnicodeNormalization;

const VARIATION_SELECTOR: char = '\u{FE0F}';
const ZERO_WIDTH_SPACE: char = '\u{200B}';
const NONCHARACTER_HYPHEN: char = '\u{FFFE}';
const TILDE_OPERATOR: char = '\u{223C}';
const DROPPED_CHARS: [char; 3] = [VARIATION_SELECTOR, ZERO_WIDTH_SPACE, NONCHARACTER_HYPHEN];

pub fn normalize_text(text: &str) -> String {
    text.nfkc().filter(|character| !DROPPED_CHARS.contains(character)).map(|character| if character == TILDE_OPERATOR { '~' } else { character }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_letters_and_ligatures_become_plain() {
        assert_eq!(normalize_text("When we 𝐬𝐚𝐯𝐞 𝐟𝐢𝐥𝐞𝐬"), "When we save files");
        assert_eq!(normalize_text("ﬁle"), "file");
    }

    #[test]
    fn invisible_chars_go_and_smart_quotes_and_unicode_hyphens_stay() {
        assert_eq!(normalize_text("▶\u{FE0F} What CSS does?"), "▶ What CSS does?");
        assert_eq!(normalize_text("a\u{200B}b ∼10 years"), "ab ~10 years");
        assert_eq!(normalize_text("it’s “quoted” back‐"), "it’s “quoted” back‐");
    }
}

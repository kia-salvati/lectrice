//! Rule 10: known junk. Calibre's "Click here to view code image" link labels and leftover print
//! page numbers like `{xxiii}`.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use super::lines::{Line, LineRole};

const CODE_IMAGE_LINK: &str = "Click here to view code image";

static PRINT_PAGE_LEFTOVER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{[xivlc]+\}").expect("valid regex"));

pub fn strip_inline_junk(text: &str) -> Cow<'_, str> {
    PRINT_PAGE_LEFTOVER.replace_all(text, "")
}

pub fn mark_junk_lines(lines: &mut [Line]) {
    for line in lines.iter_mut().filter(|line| line.is_content() && line.text.trim() == CODE_IMAGE_LINK) {
        line.role = LineRole::Junk;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_page_leftovers_are_removed() {
        assert_eq!(strip_inline_junk("machines.{xx}"), "machines.");
        assert_eq!(strip_inline_junk("{xxiii}"), "");
        assert_eq!(strip_inline_junk("a {name} stays"), "a {name} stays");
    }
}

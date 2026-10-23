//! What a font name tells us: its family and whether it is bold, italic or monospace.

const MONOSPACE_MARKERS: [&str; 7] = ["courier", "mono", "code", "consol", "menlo", "inconsolata", "fixed"];
const BOLD_MARKERS: [&str; 4] = ["bold", "black", "heavy", "semibold"];
const ITALIC_MARKERS: [&str; 2] = ["italic", "oblique"];
/// Foundry suffixes that are not part of the family name (`TimesNewRomanPSMT`, `ArialMT`).
const FAMILY_SUFFIXES: [&str; 3] = ["PSMT", "MT", "PS"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontStyle {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
}

impl FontStyle {
    pub fn parse(font_name: &str) -> Self {
        let (family_part, style_part) = font_name.split_once(['-', ',', '+']).unwrap_or((font_name, ""));
        let family = strip_family_suffix(family_part).to_string();
        let lower_name = font_name.to_ascii_lowercase();
        let lower_family = family.to_ascii_lowercase();
        Self {
            bold: BOLD_MARKERS.iter().any(|marker| lower_name.contains(marker)),
            italic: ITALIC_MARKERS.iter().any(|marker| lower_name.contains(marker)) || style_part.contains("It"),
            monospace: MONOSPACE_MARKERS.iter().any(|marker| lower_family.contains(marker)),
            family,
        }
    }
}

fn strip_family_suffix(family: &str) -> &str {
    FAMILY_SUFFIXES.iter().find_map(|suffix| family.strip_suffix(suffix).filter(|rest| !rest.is_empty())).unwrap_or(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_ignores_style_and_foundry_suffixes() {
        assert_eq!(FontStyle::parse("TimesNewRomanPSMT").family, "TimesNewRoman");
        assert_eq!(FontStyle::parse("TimesNewRomanPS-BoldMT").family, "TimesNewRoman");
        assert_eq!(FontStyle::parse("CourierNew,Bold").family, "CourierNew");
        assert_eq!(FontStyle::parse("ArialMT").family, "Arial");
    }

    #[test]
    fn styles_come_from_the_name() {
        let minion_italic = FontStyle::parse("MinionPro-It");
        assert!(minion_italic.italic && !minion_italic.bold);
        assert!(FontStyle::parse("MyriadPro-SemiboldCond").bold);
        assert!(FontStyle::parse("SourceCodePro-Regular").monospace);
        assert!(FontStyle::parse("UbuntuMono-Regular").monospace);
        assert!(FontStyle::parse("CourierNew").monospace);
        assert!(!FontStyle::parse("CrimsonText-Roman").monospace);
        assert!(!FontStyle::parse("CrimsonText-Roman").italic);
    }
}

//! Clean-up of the recognizer's Thai text.

/// Text as users type and search it: the model writes sara am as nikhahit + sara aa
/// (U+0E4D U+0E32); the combined sara am (U+0E33) is what a search for "ทำ" contains. Runs of
/// whitespace collapse to one space and the ends are trimmed.
pub fn normalize(text: &str) -> String {
    let combined = text.replace("\u{0E4D}\u{0E32}", "\u{0E33}");
    combined.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sara_am_becomes_combined() {
        assert_eq!(normalize("\u{0E17}\u{0E4D}\u{0E32}"), "\u{0E17}\u{0E33}");
        assert_eq!(normalize("ทํา"), "ทำ");
    }

    #[test]
    fn whitespace_is_collapsed_and_trimmed() {
        assert_eq!(normalize("  สวัสดี   ครับ \n"), "สวัสดี ครับ");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize(" \t\n "), "");
    }

    #[test]
    fn latin_and_digits_are_untouched() {
        assert_eq!(normalize("ok 3 ครั้ง"), "ok 3 ครั้ง");
    }

    #[test]
    fn already_combined_is_stable() {
        assert_eq!(normalize("ทำ"), "ทำ");
        assert_eq!(normalize(&normalize("ทํา")), "ทำ");
    }
}

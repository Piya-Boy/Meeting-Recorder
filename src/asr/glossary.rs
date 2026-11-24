//! Word fixes: "what the model writes = what it should be", applied to the finished text.
//!
//! One rule per line, `wrong = right`; several wrong forms are separated by commas
//! (`มอล, หมอก = มอก.`). The right side may be empty, which deletes the words. Blank lines and
//! lines that start with `#` are skipped. Latin letters match in any case; everything else
//! matches exactly. The text is read once from the left, the longest rule that matches at a
//! place wins, and what a rule wrote is never replaced again.

/// The rules of a word-fix list, longest wrong form first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Glossary {
    rules: Vec<(String, String)>,
}

enum LineKind {
    Skip,
    Bad,
    Rules(Vec<(String, String)>),
}

fn read_line(line: &str) -> LineKind {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return LineKind::Skip;
    }
    let Some((left, right)) = line.rsplit_once('=') else {
        return LineKind::Bad;
    };
    let right = right.trim();
    let wrong: Vec<&str> = left
        .split(',')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if wrong.is_empty() {
        return LineKind::Bad;
    }
    LineKind::Rules(
        wrong
            .into_iter()
            .filter(|w| *w != right)
            .map(|w| (w.to_owned(), right.to_owned()))
            .collect(),
    )
}

/// Lines of `text` that are neither a rule, a blank line nor a `#` note.
pub fn invalid_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| matches!(read_line(l), LineKind::Bad))
        .count()
}

impl Glossary {
    pub fn parse(text: &str) -> Glossary {
        let mut rules: Vec<(String, String)> = text
            .lines()
            .filter_map(|l| match read_line(l) {
                LineKind::Rules(r) => Some(r),
                _ => None,
            })
            .flatten()
            .collect();
        rules.sort_by_key(|(wrong, _)| std::cmp::Reverse(wrong.len()));
        Glossary { rules }
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn apply(&self, text: &str) -> String {
        if self.is_empty() {
            return text.to_owned();
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        'text: while i < bytes.len() {
            for (wrong, right) in &self.rules {
                let w = wrong.as_bytes();
                // Equal bytes (Latin letters in any case) end on a character boundary.
                if bytes.len() - i >= w.len() && bytes[i..i + w.len()].eq_ignore_ascii_case(w) {
                    out.push_str(right);
                    i += w.len();
                    continue 'text;
                }
            }
            let c = text[i..]
                .chars()
                .next()
                .expect("i is on a character boundary");
            out.push(c);
            i += c.len_utf8();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_variants_and_skips_bad_lines() {
        let text = "มอล, หมอก = มอก.\n\n# a note\nไม่มีเครื่องหมาย\n = x\nMax safe = MagSafe\n";
        let g = Glossary::parse(text);
        assert_eq!(g.len(), 3);
        assert_eq!(
            invalid_lines(text),
            2,
            "the line without '=' and the one with nothing on the left"
        );
        assert_eq!(g.apply("ถาม มอล กับ หมอก"), "ถาม มอก. กับ มอก.");
    }

    #[test]
    fn the_longest_match_wins_and_is_replaced_once() {
        let g = Glossary::parse("แบงค์ = bank\nพาวเวอร์แบงค์ = power bank");
        assert_eq!(g.apply("ซื้อพาวเวอร์แบงค์กับแบงค์"), "ซื้อpower bankกับbank");
    }

    #[test]
    fn a_replacement_is_not_replaced_again() {
        let g = Glossary::parse("a = b\nb = c");
        assert_eq!(g.apply("ab"), "bc");
    }

    #[test]
    fn latin_letters_match_in_any_case() {
        let g = Glossary::parse("max safe = MagSafe");
        assert_eq!(g.apply("เป็น Max Safe นะ"), "เป็น MagSafe นะ");
    }

    #[test]
    fn an_empty_right_side_deletes_the_words() {
        let g = Glossary::parse("อืม =");
        assert_eq!(g.apply("ใช่อืมครับ"), "ใช่ครับ");
    }

    #[test]
    fn no_rules_leave_the_text_alone_and_a_rule_to_itself_is_dropped() {
        assert_eq!(Glossary::default().apply("อะไร"), "อะไร");
        assert!(Glossary::parse("x = x").is_empty());
    }
}

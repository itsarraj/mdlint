//! Fenced-code-block detection and inline-code masking.
//!
//! Every rule in this tool operates on *prose*, not on code samples that
//! happen to be embedded in the document. A Python comment (`# not a
//! heading`) inside a ```` ```python ```` block isn't a Markdown heading; a
//! `curl https://example.com` inside a shell snippet isn't a bare URL that
//! needs wrapping. So before any rule runs, we compute which 1-based line
//! numbers fall inside a fenced code block, and provide a helper to mask
//! out inline (single-backtick) code spans within a single line.

/// Returns the set of 1-based line numbers that fall inside a fenced code
/// block, including the fence lines themselves (open and close).
///
/// A fence is a line whose trimmed content starts with 3+ backticks or 3+
/// tildes. A block is closed by a later line using the *same* fence
/// character with a run at least as long as the opening one (this matches
/// CommonMark's fence-matching rule closely enough for real documents). An
/// unterminated fence runs to end of file.
pub fn fenced_line_numbers(source: &str) -> std::collections::HashSet<usize> {
    let mut in_fence: Option<(char, usize)> = None;
    let mut fenced = std::collections::HashSet::new();

    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim_start();

        match in_fence {
            None => {
                if let Some((ch, len)) = fence_marker(trimmed) {
                    in_fence = Some((ch, len));
                    fenced.insert(line_no);
                }
            }
            Some((open_ch, open_len)) => {
                fenced.insert(line_no);
                if let Some((ch, len)) = fence_marker(trimmed) {
                    if ch == open_ch && len >= open_len {
                        in_fence = None;
                    }
                }
            }
        }
    }

    fenced
}

/// If `trimmed` starts with a run of 3+ `` ` `` or `~`, returns the
/// character and run length. A fence line may have trailing "info string"
/// text after the run (a language tag like `` ```rust ``), which is
/// ignored for matching purposes.
fn fence_marker(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|c| *c == first).count();
    if len >= 3 {
        Some((first, len))
    } else {
        None
    }
}

/// Replaces the contents of every inline (single-backtick) code span in
/// `line` with spaces of the same byte length, so a rule that scans for
/// patterns (like a bare URL) won't match text that's actually inside
/// `` `code` ``. Backtick count and escaping rules aren't fully
/// CommonMark-accurate — this handles the common single-backtick case,
/// which covers real-world Markdown prose.
pub fn mask_inline_code(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out: Vec<u8> = bytes.to_vec();
    let mut i = 0;
    let mut open: Option<usize> = None;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            match open {
                None => open = Some(i),
                Some(start) => {
                    for b in out.iter_mut().take(i + 1).skip(start) {
                        if *b != b'`' {
                            *b = b' ';
                        }
                    }
                    open = None;
                }
            }
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_simple_fenced_block() {
        let src = "text\n```\ncode\n```\nmore text";
        let fenced = fenced_line_numbers(src);
        assert!(fenced.contains(&2));
        assert!(fenced.contains(&3));
        assert!(fenced.contains(&4));
        assert!(!fenced.contains(&1));
        assert!(!fenced.contains(&5));
    }

    #[test]
    fn detects_tilde_fences_with_a_language_tag() {
        let src = "~~~python\n# comment\n~~~";
        let fenced = fenced_line_numbers(src);
        assert_eq!(fenced.len(), 3);
    }

    #[test]
    fn unterminated_fence_runs_to_eof() {
        let src = "```\nline one\nline two";
        let fenced = fenced_line_numbers(src);
        assert_eq!(fenced.len(), 3);
    }

    #[test]
    fn mask_inline_code_blanks_backtick_spans_only() {
        let masked = mask_inline_code("see `https://example.com` for docs");
        assert!(!masked.contains("https://example.com"));
        assert!(masked.contains("see"));
        assert!(masked.contains("for docs"));
        // Length is preserved so later byte-offset math still lines up.
        assert_eq!(masked.len(), "see `https://example.com` for docs".len());
    }

    #[test]
    fn mask_inline_code_leaves_plain_lines_untouched() {
        let masked = mask_inline_code("no code spans here");
        assert_eq!(masked, "no code spans here");
    }
}

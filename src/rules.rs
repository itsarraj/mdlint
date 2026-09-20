use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::fences::{fenced_line_numbers, mask_inline_code};

pub struct Finding {
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

static ATX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})(?:\s+(.*?))?\s*#*\s*$").unwrap());

// A run of `-`, `*` or `+`, or a digit-dot ordered marker, followed by
// whitespace — a list item marker per CommonMark.
static LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\t ]*([-*+]|\d+[.)])[ \t]+\S").unwrap());

static BARE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://[^\s<>()\[\]]+").unwrap());

/// A parsed ATX heading: its level (1-6), trimmed text, and 1-based line.
struct Heading {
    level: usize,
    text: String,
    line: usize,
}

fn parse_headings(source: &str, fenced: &std::collections::HashSet<usize>) -> Vec<Heading> {
    let mut out = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        if fenced.contains(&line_no) {
            continue;
        }
        if let Some(caps) = ATX_HEADING.captures(line) {
            let level = caps[1].len();
            let text = caps
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            out.push(Heading {
                level,
                text,
                line: line_no,
            });
        }
    }
    out
}

/// MD001-style rule: a heading's level should never jump by more than one
/// step past the immediately preceding heading (`#` then `###` with no
/// `##` in between). Comparing to the previous heading in document order
/// — not a nesting stack — matches how markdownlint's own `heading-
/// increment` rule behaves, and how a reader actually experiences the
/// document top to bottom.
fn check_heading_increment(headings: &[Heading]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut prev: Option<usize> = None;
    for h in headings {
        if let Some(p) = prev {
            if h.level > p + 1 {
                findings.push(Finding {
                    line: h.line,
                    rule: "heading-increment",
                    message: format!(
                        "heading level jumps from h{p} to h{level} — no h{mid} in between",
                        level = h.level,
                        mid = p + 1
                    ),
                });
            }
        }
        prev = Some(h.level);
    }
    findings
}

/// Two headings at the *same* level with identical (trimmed) text — a
/// reader following a link like `#section` or a table of contents can no
/// longer tell which one is meant.
fn check_duplicate_headings(headings: &[Heading]) -> Vec<Finding> {
    let mut seen: HashMap<(usize, String), usize> = HashMap::new();
    let mut findings = Vec::new();
    for h in headings {
        if h.text.is_empty() {
            continue;
        }
        let key = (h.level, h.text.clone());
        if let Some(first_line) = seen.get(&key) {
            findings.push(Finding {
                line: h.line,
                rule: "duplicate-heading",
                message: format!(
                    "duplicate h{} heading text \"{}\" — first used on line {first_line}",
                    h.level, h.text
                ),
            });
        } else {
            seen.insert(key, h.line);
        }
    }
    findings
}

/// MD009-style rule: trailing whitespace is usually accidental. A run of
/// exactly two trailing spaces is CommonMark's own hard-line-break syntax
/// and is left alone; a tab, a single space, or 3+ spaces is flagged.
fn check_trailing_whitespace(
    source: &str,
    fenced: &std::collections::HashSet<usize>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        if fenced.contains(&line_no) {
            continue;
        }
        let trimmed = line.trim_end_matches([' ', '\t']);
        let trailing = &line[trimmed.len()..];
        if trailing.is_empty() {
            continue;
        }
        if trailing.contains('\t') {
            findings.push(Finding {
                line: line_no,
                rule: "trailing-whitespace",
                message: "trailing whitespace includes a hard tab".to_string(),
            });
        } else if trailing.len() != 2 {
            findings.push(Finding {
                line: line_no,
                rule: "trailing-whitespace",
                message: format!(
                    "{} trailing space(s) — use exactly 2 for a hard line break, or none",
                    trailing.len()
                ),
            });
        }
    }
    findings
}

/// MD034-style rule: a raw `http(s)://` URL sitting in prose, not wrapped
/// in `<...>` and not the destination of a `[text](url)` link, renders
/// inconsistently across Markdown flavors (some autolink it, some don't)
/// and is easy to mistake for plain text in a diff.
fn check_bare_urls(source: &str, fenced: &std::collections::HashSet<usize>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        if fenced.contains(&line_no) {
            continue;
        }
        let masked = mask_inline_code(line);
        for m in BARE_URL.find_iter(&masked) {
            let start = m.start();
            let end = m.end();

            let angle_wrapped =
                start > 0 && masked.as_bytes()[start - 1] == b'<' && masked[end..].starts_with('>');
            let md_link = start >= 2 && &masked[start - 2..start] == "](";

            if angle_wrapped || md_link {
                continue;
            }

            findings.push(Finding {
                line: line_no,
                rule: "bare-url",
                message: format!(
                    "bare URL `{}` — wrap it in <...> or use [text]({}) link syntax",
                    m.as_str(),
                    m.as_str()
                ),
            });
        }
    }
    findings
}

/// MD005/MD007-style rule: list nesting is conventionally expressed with
/// spaces. A literal tab in a list item's leading indentation renders
/// inconsistently across editors/renderers (tab-width 2 vs 4 vs 8 changes
/// which parent item it visually nests under).
fn check_hard_tab_list_indent(
    source: &str,
    fenced: &std::collections::HashSet<usize>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        if fenced.contains(&line_no) {
            continue;
        }
        if !LIST_MARKER.is_match(line) {
            continue;
        }
        let leading: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if leading.contains('\t') {
            findings.push(Finding {
                line: line_no,
                rule: "hard-tab-list-indent",
                message: "list item indented with a hard tab — use spaces so nesting renders consistently".to_string(),
            });
        }
    }
    findings
}

/// Runs every rule over a document's full source text and returns all
/// findings, sorted by line number.
pub fn lint(source: &str) -> Vec<Finding> {
    let fenced = fenced_line_numbers(source);
    let headings = parse_headings(source, &fenced);

    let mut findings = Vec::new();
    findings.extend(check_heading_increment(&headings));
    findings.extend(check_duplicate_headings(&headings));
    findings.extend(check_trailing_whitespace(source, &fenced));
    findings.extend(check_bare_urls(source, &fenced));
    findings.extend(check_hard_tab_list_indent(source, &fenced));
    findings.sort_by_key(|f| f.line);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_heading_level_jump() {
        let f = lint("# Title\n\n### Section\n");
        assert!(f
            .iter()
            .any(|x| x.rule == "heading-increment" && x.line == 3));
    }

    #[test]
    fn does_not_flag_a_normal_heading_progression() {
        let f = lint("# Title\n\n## Section\n\n### Sub\n");
        assert!(!f.iter().any(|x| x.rule == "heading-increment"));
    }

    #[test]
    fn does_not_flag_heading_level_decreasing() {
        // Going back down (h3 -> h1) then up by one step is fine; only an
        // *upward* jump of more than one level is the MD001 violation.
        let f = lint("# A\n\n## B\n\n### C\n\n# D\n\n## E\n");
        assert!(!f.iter().any(|x| x.rule == "heading-increment"));
    }

    #[test]
    fn flags_duplicate_heading_at_same_level() {
        let f = lint("## Section A\n\ntext\n\n## Section A\n");
        let dup = f.iter().find(|x| x.rule == "duplicate-heading").unwrap();
        assert_eq!(dup.line, 5);
        assert!(dup.message.contains("first used on line 1"));
    }

    #[test]
    fn does_not_flag_same_text_at_different_levels() {
        let f = lint("# Overview\n\n## Overview\n");
        assert!(!f.iter().any(|x| x.rule == "duplicate-heading"));
    }

    #[test]
    fn flags_trailing_single_space() {
        let f = lint("line with one trailing space \n");
        assert!(f
            .iter()
            .any(|x| x.rule == "trailing-whitespace" && x.line == 1));
    }

    #[test]
    fn allows_exactly_two_trailing_spaces_as_a_hard_break() {
        let f = lint("line with a hard break  \nnext line\n");
        assert!(!f.iter().any(|x| x.rule == "trailing-whitespace"));
    }

    #[test]
    fn flags_trailing_tab() {
        let f = lint("line with a trailing tab\t\n");
        assert!(f.iter().any(|x| x.rule == "trailing-whitespace"));
    }

    #[test]
    fn flags_bare_url() {
        let f = lint("see https://example.com for more\n");
        assert!(f.iter().any(|x| x.rule == "bare-url"));
    }

    #[test]
    fn does_not_flag_angle_wrapped_url() {
        let f = lint("see <https://example.com> for more\n");
        assert!(!f.iter().any(|x| x.rule == "bare-url"));
    }

    #[test]
    fn does_not_flag_markdown_link_syntax() {
        let f = lint("see [the docs](https://example.com) for more\n");
        assert!(!f.iter().any(|x| x.rule == "bare-url"));
    }

    #[test]
    fn does_not_flag_url_inside_inline_code() {
        let f = lint("run `curl https://example.com/health`\n");
        assert!(!f.iter().any(|x| x.rule == "bare-url"));
    }

    #[test]
    fn flags_tab_indented_nested_list_item() {
        let f = lint("- top\n\t- nested\n");
        assert!(f
            .iter()
            .any(|x| x.rule == "hard-tab-list-indent" && x.line == 2));
    }

    #[test]
    fn does_not_flag_space_indented_nested_list_item() {
        let f = lint("- top\n  - nested\n");
        assert!(!f.iter().any(|x| x.rule == "hard-tab-list-indent"));
    }

    #[test]
    fn does_not_flag_top_level_list_items() {
        let f = lint("- one\n- two\n- three\n");
        assert!(!f.iter().any(|x| x.rule == "hard-tab-list-indent"));
    }

    #[test]
    fn fenced_code_block_is_immune_to_every_rule() {
        let doc = "# Title\n\n```text\n# not a heading\nhttps://example.com/not-flagged\n\t- not list indentation, just code\n```\n";
        let f = lint(doc);
        assert!(f.is_empty());
    }

    #[test]
    fn clean_document_has_no_findings() {
        let doc = "# Title\n\nSome intro text with a [proper link](https://example.com) and\nan angle-wrapped one <https://example.org>.\n\n## Section One\n\n- item\n  - nested item\n\n## Section Two\n\n### Subsection\n";
        let f = lint(doc);
        assert!(
            f.is_empty(),
            "expected no findings, got: {:?}",
            f.iter().map(|x| (x.line, x.rule)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn heading_text_extraction_ignores_trailing_atx_closing_hashes() {
        // Closed-ATX style (`# Title #`) is valid CommonMark; the trailing
        // `#`s are decoration, not part of the heading text. If they
        // weren't stripped, these two h1s would compare as different text
        // ("Title #" vs "Title") and the real duplicate would be missed.
        let f = lint("# Title #\n\n# Title\n");
        assert!(f
            .iter()
            .any(|x| x.rule == "duplicate-heading" && x.line == 3));
    }
}

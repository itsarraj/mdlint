# mdlint

Lints Markdown files for the style issues that actually cause real
rendering problems — no Node/npm required, unlike `markdownlint-cli2`
(the dominant tool for this today).

## Installation

```sh
cargo install --path .
```

## Usage

```bash
mdlint README.md
mdlint docs/*.md
```

Exit code `1` if anything's found, `0` if clean.

## The five rules

- **`heading-increment`**: a heading level jumps by more than one (`#`
  straight to `###`, no `##` in between) — breaks a generated table of
  contents and screen-reader navigation.
- **`trailing-whitespace`**: trailing spaces on a line. Exactly two is
  CommonMark's actual hard-line-break syntax and is left alone; any
  other count is almost always an accidental artifact.
- **`bare-url`**: a bare `http://`/`https://` URL not wrapped in
  `<...>` or `[text](...)` link syntax — renders inconsistently across
  Markdown flavors (some auto-link it, some don't).
- **`duplicate-heading`**: two headings at the same level with identical
  text — breaks anchor-link uniqueness (`#section` can only point at
  one of them).
- **`hard-tab-list-indent`**: a nested list item indented with a literal
  tab instead of spaces — renders inconsistently across Markdown
  parsers, some of which treat a tab as a fixed number of spaces and
  some of which don't nest it at all.

Code fences (` ``` `) and inline code spans are masked before any rule
runs, so a URL or heading-looking text inside a code block is never
flagged.

## Status: built and verified against a realistic fixture hitting every rule, and its clean counterpart

- **23 unit tests** (`cargo test --lib`) across `rules` (each of the
  five rules in isolation, including the negative case for each — a
  space-indented nested list correctly not flagged, a real hard line
  break with exactly two trailing spaces correctly not flagged, a
  non-consecutive heading jump like h2→h1→h3 correctly not
  double-flagged) and `fences` (code-fence detection and inline-code
  masking, so rule content inside a fenced block or inline span is
  correctly ignored).
- **Live-verified against the actual compiled binary and two realistic
  files**: a fixture deliberately hitting all five rules (an h1→h3
  jump, trailing whitespace, a bare URL, a duplicate `## Section`
  heading, and a tab-indented nested list item) produced exactly the 5
  expected findings with correct line numbers and exit code `1`; a
  clean file using the correct form of each (proper heading sequence, a
  markdown-link instead of a bare URL, uniquely-named sections,
  space-indented nesting) produced "no issues found" and exit code `0`.

**Not done / deliberately deferred**: line-length limits (a common
markdownlint rule, deliberately left out — line length is a style
preference with far more legitimate disagreement than the five rules
above, which are closer to "this actually renders wrong somewhere");
front-matter (YAML `---` blocks at the top of a file) parsing — front
matter isn't stripped before linting, so a heading-shaped line inside it
would be misread as a real heading; and Setext-style headings
(`Title\n=====` underline syntax) — only ATX (`#`-prefixed) headings are
recognized.

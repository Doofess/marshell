//! The text checks behind the Doctor's instruction findings (slice 7d2): backticked paths, repeated blocks, package
//! managers, indents and the import fix. Everything here is pure, and nothing here logs: a quoted line is file
//! content, and the logs are exported (decision 14).
//!
//! No check but `remove_imports` with `in_comments` reads inside an HTML comment: each runs on the text with its
//! comments masked (`mask_html_comments` keeps byte lengths and line numbers).

use std::collections::HashSet;

use super::instructions::{ImportAt, ImportSyntax, code_mask, imports_at, is_fence, mask_html_comments};

const QUOTE_MAX: usize = 80;

/// At most 80 characters of a line, trimmed, with "…" when cut (the "…" is one of the 80).
pub fn quote(line: &str) -> String {
    let line = line.trim();
    if line.chars().count() <= QUOTE_MAX {
        return line.to_string();
    }
    let cut: String = line.chars().take(QUOTE_MAX - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The numbered lines of a text that sit outside fenced blocks (the fence lines themselves are left out too).
fn outside_fences(text: &str) -> Vec<(usize, &str)> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if is_fence(line) {
            in_fence = !in_fence;
        } else if !in_fence {
            out.push((i + 1, line));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Backticked paths (decision 4)

/// The contents of the inline code spans of one line.
fn code_spans(chars: &[char]) -> Vec<String> {
    let mask = code_mask(chars);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !mask[i] {
            i += 1;
            continue;
        }
        let mut end = i;
        while end < chars.len() && mask[end] {
            end += 1;
        }
        let run = &chars[i..end];
        let mut k = 0;
        while k < run.len() {
            let n = run[k..].iter().take_while(|c| **c == '`').count();
            if n == 0 {
                break;
            }
            let mut j = k + n;
            let mut close = None;
            while j < run.len() {
                if run[j] == '`' {
                    let len = run[j..].iter().take_while(|c| **c == '`').count();
                    if len == n {
                        close = Some(j);
                        break;
                    }
                    j += len;
                } else {
                    j += 1;
                }
            }
            let Some(close) = close else { break };
            out.push(run[k + n..close].iter().collect());
            k = close + n;
        }
        i = end;
    }
    out
}

/// The relative path a code span holds, with a trailing `#anchor`, `:line` or `:line:col` dropped, or None.
fn path_of(span: &str) -> Option<String> {
    let mut path = span.trim();
    path = path.split('#').next().unwrap_or("");
    for _ in 0..2 {
        match path.rfind(':') {
            Some(at) if at > 0 && !path[at + 1..].is_empty() && path[at + 1..].bytes().all(|b| b.is_ascii_digit()) => {
                path = &path[..at];
            }
            _ => break,
        }
    }
    if path.is_empty() || !path.contains('/') {
        return None;
    }
    if path
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '*' | '?' | '[' | '{' | '<' | '$' | '%'))
    {
        return None;
    }
    if path.contains("://") || path.contains("...") {
        return None;
    }
    if path.starts_with("path/to/") || path.starts_with(['/', '~', '-', '@']) {
        return None;
    }
    let mut chars = path.chars();
    if chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':') {
        return None;
    }
    Some(path.to_string())
}

/// Backticked relative paths (decision 4), with their 1-based line, in order, each path once.
pub fn backticked_paths(text: &str) -> Vec<(usize, String)> {
    let masked = mask_html_comments(text);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (number, line) in outside_fences(&masked) {
        let chars: Vec<char> = line.chars().collect();
        for span in code_spans(&chars) {
            if let Some(path) = path_of(&span)
                && seen.insert(path.clone())
            {
                out.push((number, path));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Repeated blocks (decision 5)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub line: usize,
    pub folded: String,
    pub quote: String,
    pub chars: usize,
}

const BLOCK_MIN: usize = 200;

fn block_of(lines: &[(usize, &str)], fenced: bool, out: &mut Vec<Block>) {
    let Some(&(line, first)) = lines.first() else { return };
    let joined = lines.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(" ");
    let folded = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = folded.chars().count();
    if chars < BLOCK_MIN {
        return;
    }
    // A fence's own first line is only "```": quote the first line with something in it.
    let shown = if fenced {
        lines
            .iter()
            .skip(1)
            .map(|(_, l)| *l)
            .find(|l| !l.trim().is_empty() && !is_fence(l))
            .unwrap_or(first)
    } else {
        first
    };
    out.push(Block {
        line,
        folded,
        quote: quote(shown),
        chars,
    });
}

/// Paragraphs and fenced blocks of 200 characters or more (decision 5). `folded` has every run of whitespace as one
/// space.
pub fn blocks(text: &str) -> Vec<Block> {
    let masked = mask_html_comments(text);
    let mut out = Vec::new();
    let mut paragraph: Vec<(usize, &str)> = Vec::new();
    let mut fence: Option<Vec<(usize, &str)>> = None;
    for (i, line) in masked.lines().enumerate() {
        let number = i + 1;
        if is_fence(line) {
            match fence.take() {
                Some(mut open) => {
                    open.push((number, line));
                    block_of(&open, true, &mut out);
                }
                None => {
                    block_of(&paragraph, false, &mut out);
                    paragraph.clear();
                    fence = Some(vec![(number, line)]);
                }
            }
        } else if let Some(open) = fence.as_mut() {
            open.push((number, line));
        } else if line.trim().is_empty() {
            block_of(&paragraph, false, &mut out);
            paragraph.clear();
        } else {
            paragraph.push((number, line));
        }
    }
    block_of(&paragraph, false, &mut out);
    if let Some(open) = fence {
        block_of(&open, true, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Words, shared by the package-manager and indent rules

/// A line as lowercase ASCII letters (other characters kept), so that an index in it is an index in the line.
fn lower_chars(line: &str) -> Vec<char> {
    line.chars().map(|c| c.to_ascii_lowercase()).collect()
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '’'
}

/// Words: runs of letters and digits, with an apostrophe kept inside one ("don't").
fn words(chars: &[char]) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len()
            && (chars[i].is_alphanumeric()
                || (is_apostrophe(chars[i]) && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric())))
        {
            i += 1;
        }
        out.push((start, i, chars[start..i].iter().collect()));
    }
    out
}

const NEGATIONS: [&str; 6] = ["not", "don't", "don’t", "never", "avoid", "no"];

/// The number of the first word of a line that negates what comes after it: "not", "don't", "never", "avoid", "no",
/// or the "of" of "instead of". Worked out once for a line, so that asking about each of its words stays linear.
fn first_negation(list: &[(usize, usize, String)]) -> Option<usize> {
    list.iter().enumerate().position(|(k, (_, _, w))| {
        NEGATIONS.contains(&w.as_str()) || (k > 0 && w == "of" && list[k - 1].2 == "instead")
    })
}

/// Whether word number `at` comes after a negation on its line (`first_negation`).
fn negated(first: Option<usize>, at: usize) -> bool {
    first.is_some_and(|negation| negation < at)
}

/// Where the last `-g` or `--global` of a line starts, as an index in its characters. Worked out once for a line.
fn last_global_flag(chars: &[char]) -> Option<usize> {
    let mut last = None;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let token = &chars[start..i];
        if matches!(token, ['-', 'g']) || token.iter().copied().eq("--global".chars()) {
            last = Some(start);
        }
    }
    last
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said<T> {
    pub line: usize,
    pub what: T,
    pub quote: String,
}

// ---------------------------------------------------------------------------------------------------------------
// Package managers (decision 12)

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Manager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl Manager {
    pub fn name(self) -> &'static str {
        match self {
            Manager::Npm => "npm",
            Manager::Pnpm => "pnpm",
            Manager::Yarn => "yarn",
            Manager::Bun => "bun",
        }
    }
}

const MANAGERS: [Manager; 4] = [Manager::Npm, Manager::Pnpm, Manager::Yarn, Manager::Bun];
const VERBS: [&str; 11] = [
    "install", "i", "add", "ci", "run", "test", "exec", "dlx", "build", "dev", "start",
];

/// Where a word stands alone in a line: `pnpm-lock.yaml`, `bunx`, `bundle`, `@scope/npm` and `npm.cmd` are not it.
fn stands_alone(list: &[(usize, usize, String)], at: usize, chars: &[char]) -> bool {
    let (start, end, _) = &list[at];
    let before_ok = *start == 0 || !matches!(chars[start - 1], '-' | '_' | '.' | '/' | '@' | '\\');
    let after_ok = match chars.get(*end) {
        None => true,
        Some('-' | '_' | '/' | '\\') => false,
        Some('.') => !chars.get(end + 1).is_some_and(|c| c.is_alphanumeric() || *c == '_'),
        Some(_) => true,
    };
    before_ok && after_ok
}

/// Whether the word before word `at` is "use", "uses" or "using" (with one "the" allowed between).
fn used(list: &[(usize, usize, String)], at: usize, chars: &[char]) -> bool {
    let is_use = |w: &str| matches!(w, "use" | "uses" | "using");
    // Only blanks and quote marks may sit between the two words.
    let joined = |before: usize, after: usize| {
        chars[list[before].1..list[after].0]
            .iter()
            .all(|c| c.is_whitespace() || matches!(c, '`' | '\'' | '"' | '('))
    };
    if at >= 1 && joined(at - 1, at) && is_use(&list[at - 1].2) {
        return true;
    }
    at >= 2 && list[at - 1].2 == "the" && joined(at - 1, at) && joined(at - 2, at - 1) && is_use(&list[at - 2].2)
}

/// Lines that name a package manager for use (decision 12). Capitals don't matter.
pub fn package_managers(text: &str) -> Vec<Said<Manager>> {
    let masked = mask_html_comments(text);
    let mut out = Vec::new();
    for (i, line) in masked.lines().enumerate() {
        let chars = lower_chars(line);
        let list = words(&chars);
        if list.is_empty() {
            continue;
        }
        let lowered: String = chars.iter().collect();
        let package_manager = lowered.contains("package manager");
        // What depends on the whole line is worked out once, not for each manager word: a line of many of them
        // (a minified file, a generated list) would otherwise be read in quadratic time.
        let negation = first_negation(&list);
        let global_flag = last_global_flag(&chars);
        let mut named: Vec<Manager> = Vec::new();
        for (at, (_, end, word)) in list.iter().enumerate() {
            let Some(manager) = MANAGERS.iter().copied().find(|m| m.name() == word) else {
                continue;
            };
            if named.contains(&manager) || !stands_alone(&list, at, &chars) || negated(negation, at) {
                continue;
            }
            let global = global_flag.is_some_and(|start| start >= *end);
            let verb = !global
                && list.get(at + 1).is_some_and(|(next, _, w)| {
                    VERBS.contains(&w.as_str()) && *next > *end && chars[*end..*next].iter().all(|c| c.is_whitespace())
                });
            if verb || used(&list, at, &chars) || package_manager {
                named.push(manager);
            }
        }
        for manager in named {
            out.push(Said {
                line: i + 1,
                what: manager,
                quote: quote(line),
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Indents (decision 13)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentKind {
    Tabs,
    Spaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indent {
    pub kind: Option<IndentKind>,
    pub width: Option<u8>,
}

/// Words that may sit between "indent" and its width: "indent of 4", "indent width 8".
const WIDTH_LINKS: [&str; 9] = ["of", "width", "size", "level", "is", "to", "at", "by", "with"];

fn indent_of_line(line: &str) -> Option<Indent> {
    let chars = lower_chars(line);
    let list = words(&chars);
    if !list.iter().any(|(_, _, w)| w.contains("indent")) {
        return None;
    }
    let is_space = |w: &str| matches!(w, "space" | "spaces");
    let mut tabs = false;
    let mut spaces = false;
    let negation = first_negation(&list);
    for (at, (_, _, w)) in list.iter().enumerate() {
        if matches!(w.as_str(), "tab" | "tabs") && !negated(negation, at) {
            tabs = true;
        }
        if is_space(w) && !negated(negation, at) {
            spaces = true;
        }
    }
    let kind = match (tabs, spaces) {
        (true, false) => Some(IndentKind::Tabs),
        (false, true) => Some(IndentKind::Spaces),
        _ => None,
    };
    // `tabs` and `spaces` above are true only for a word that isn't negated, so "tabs or spaces" has both: it asks
    // for neither.
    let mut widths: Vec<u8> = Vec::new();
    for (at, (_, end, w)) in list.iter().enumerate() {
        if w.is_empty() || !w.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(value) = w.parse::<u8>().ok().filter(|v| (1..=8).contains(v)) else {
            continue;
        };
        let before_spaces = list.get(at + 1).is_some_and(|(next, _, n)| {
            is_space(n) && chars[*end..*next].iter().all(|c| matches!(c, ' ' | '-')) && *next - *end <= 1
        });
        let mut back = at;
        while back > 0 && WIDTH_LINKS.contains(&list[back - 1].2.as_str()) {
            back -= 1;
        }
        // Only blanks and `:`, `=`, `,`, `-` may sit between the words of "indent width: 8".
        let after_indent = back > 0
            && list[back - 1].2.contains("indent")
            && (back - 1..at).all(|k| {
                let gap = &chars[list[k].1..list[k + 1].0];
                !gap.is_empty()
                    && gap
                        .iter()
                        .all(|c| c.is_whitespace() || matches!(c, ':' | '=' | ',' | '-'))
            });
        if (before_spaces || after_indent) && !widths.contains(&value) {
            widths.push(value);
        }
    }
    let width = if widths.len() == 1 { Some(widths[0]) } else { None };
    (kind.is_some() || width.is_some()).then_some(Indent { kind, width })
}

/// Lines that ask for an indent (decision 13). A line gives at least one of `kind` and `width`.
pub fn indents(text: &str) -> Vec<Said<Indent>> {
    let masked = mask_html_comments(text);
    outside_fences(&masked)
        .into_iter()
        .filter_map(|(number, line)| {
            indent_of_line(line).map(|what| Said {
                line: number,
                what,
                quote: quote(line),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// The import fix (decision 2)

/// The text without every import written as one of `raws` (decision 2). With `in_comments`, the ones inside HTML
/// comments go too (decision 17). None when nothing was removed.
///
/// A line that holds only the import (a list marker aside) goes with its line end; otherwise the import goes with one
/// space after it, or, at a line's end, with one space before it. Every other byte stays as it was: the line ends, a
/// BOM, non-ASCII text, a missing final newline.
pub fn remove_imports(text: &str, raws: &[String], syntax: ImportSyntax, in_comments: bool) -> Option<String> {
    let masked;
    let searched: &str = if in_comments {
        text
    } else {
        masked = mask_html_comments(text);
        &masked
    };
    let found: Vec<ImportAt> = imports_at(searched, syntax)
        .into_iter()
        .filter(|at| raws.contains(&at.raw))
        .collect();
    if found.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    let mut number = 0;
    while pos < text.len() {
        number += 1;
        let newline = text[pos..].find('\n');
        let line_end = newline.map_or(text.len(), |n| pos + n + 1);
        let mut content_end = newline.map_or(text.len(), |n| pos + n);
        if newline.is_some() && text[pos..content_end].ends_with('\r') {
            content_end -= 1;
        }
        let ranges: Vec<(usize, usize)> = found
            .iter()
            .filter(|at| at.line == number)
            .map(|at| (at.start, at.end))
            .collect();
        if ranges.is_empty() {
            out.push_str(&text[pos..line_end]);
            pos = line_end;
            continue;
        }
        let line = &text[pos..content_end];
        let mut rest = String::new();
        let mut from = 0;
        for (start, end) in &ranges {
            rest.push_str(&line[from..*start]);
            from = *end;
        }
        rest.push_str(&line[from..]);
        // A BOM is no content: the first line may hold only it besides the import, and it stays.
        let bom = number == 1 && line.starts_with('\u{feff}');
        if matches!(rest.trim_start_matches('\u{feff}').trim(), "" | "-" | "*" | "+") {
            if bom {
                out.push('\u{feff}');
            }
            pos = line_end;
            continue;
        }
        let mut kept = line.to_string();
        for (start, end) in ranges.iter().rev() {
            let (mut cut_start, mut cut_end) = (*start, *end);
            match kept.as_bytes().get(*end) {
                Some(b' ') => cut_end += 1,
                None if *start > 0 && kept.as_bytes()[*start - 1] == b' ' => cut_start -= 1,
                _ => {}
            }
            kept.replace_range(cut_start..cut_end, "");
        }
        out.push_str(&kept);
        out.push_str(&text[content_end..line_end]);
        pos = line_end;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backticked_paths_are_relative_paths_with_a_slash() {
        let text = "`src/a.ts` `src/a.ts:12` `docs/x.md#top` `src/a.ts`\n\
`pnpm` `README.md` `src/*.ts` `<root>/x` `$HOME/x` `%TEMP%/x` `https://a/b` `a/.../b` `path/to/file.ts` `/abs/x` `~/x` `C:/x/y` `-r/x` `@scope/pkg` `a b/c`\n\
```\n`in/fence.ts`\n```\n<!-- `in/comment.ts` -->\n";
        assert_eq!(
            backticked_paths(text),
            vec![(1, "src/a.ts".to_string()), (1, "docs/x.md".to_string())]
        );
    }

    #[test]
    fn blocks_are_paragraphs_of_200_characters_or_more() {
        let short = "a".repeat(199);
        assert!(blocks(&short).is_empty());

        let exact = format!("intro\n\n{}\n", "a".repeat(200));
        let found = blocks(&exact);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 3);
        assert_eq!(found[0].chars, 200);

        let a = "a".repeat(100);
        let b = "b".repeat(50);
        let c = "c".repeat(50);
        let first = blocks(&format!("{a} {b}\n{c}\n"));
        let second = blocks(&format!("{a}  {b} {c}\n"));
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].folded, second[0].folded);

        let fenced = blocks(&format!("```\n{}\n```\n", "x".repeat(250)));
        assert_eq!(fenced.len(), 1);
        assert_eq!(fenced[0].line, 1);

        let long_first = format!("{}\n{}\n", "q".repeat(120), "r".repeat(100));
        let cut = blocks(&long_first);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].quote, quote(&"q".repeat(120)));
        assert_eq!(cut[0].quote.chars().count(), 80);
        assert!(cut[0].quote.ends_with('…'));

        let hidden = format!("<!--\n{}\n-->\n", "z".repeat(300));
        assert!(blocks(&hidden).is_empty());
    }

    fn managers(text: &str) -> Vec<Manager> {
        package_managers(text).into_iter().map(|s| s.what).collect()
    }

    #[test]
    fn package_managers_named_for_use() {
        for (line, want) in [
            ("Use pnpm for the frontend.", vec![Manager::Pnpm]),
            ("Run npm install first", vec![Manager::Npm]),
            ("yarn add react", vec![Manager::Yarn]),
            ("The package manager is bun.", vec![Manager::Bun]),
            ("Use Bun here.", vec![Manager::Bun]),
            ("use pnpm instead of npm", vec![Manager::Pnpm]),
            ("```\npnpm test\n```", vec![Manager::Pnpm]),
            ("npx vitest", vec![]),
            ("bunx tsc", vec![]),
            ("npm install -g typescript", vec![]),
            ("Don't use npm.", vec![]),
            ("never run yarn install", vec![]),
            ("pnpm, not npm", vec![]),
            ("the pnpm-lock.yaml file", vec![]),
            ("bundle the app", vec![]),
            ("npm", vec![]),
        ] {
            assert_eq!(managers(line), want, "{line}");
        }
        let said = package_managers("a\nUse pnpm for the frontend.\n");
        assert_eq!(said[0].line, 2);
        assert_eq!(said[0].quote, "Use pnpm for the frontend.");
        assert_eq!(Manager::Pnpm.name(), "pnpm");
    }

    /// How long `work` takes, and what it returned.
    fn timed<T>(work: impl FnOnce() -> T) -> (T, std::time::Duration) {
        let start = std::time::Instant::now();
        let out = work();
        (out, start.elapsed())
    }

    #[test]
    fn one_long_line_of_manager_words_is_read_in_linear_time() {
        // 300,000 words that name nothing for use, then one that does: every word before it was once checked
        // against all the words before it and the rest of the line.
        let line = format!("{}install", "npm ".repeat(300_000));
        let (said, took) = timed(|| package_managers(&line));
        eprintln!("package_managers over one long line took {took:?}");
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].what, Manager::Npm);
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
        let words = "npm ".repeat(300_000);
        let (said, took) = timed(|| package_managers(&words));
        assert!(said.is_empty());
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    }

    #[test]
    fn one_long_line_of_indent_words_is_read_in_linear_time() {
        let line = format!("Indent with {}", "tabs ".repeat(300_000));
        let (said, took) = timed(|| indents(&line));
        eprintln!("indents over one long line took {took:?}");
        assert_eq!(said.len(), 1);
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    }

    #[test]
    fn one_line_of_backtick_runs_of_every_length_is_read_in_linear_time() {
        // Runs of 1, 2, 3, … backticks that never close: each opener once scanned to the line's end.
        let line: String = (1..=2_000).map(|n| format!("{} ", "`".repeat(n))).collect();
        let (paths, took) = timed(|| backticked_paths(&line));
        eprintln!("backticked_paths over one long line took {took:?}");
        assert!(paths.is_empty());
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    }

    fn indent_of(text: &str) -> Vec<(Option<IndentKind>, Option<u8>)> {
        indents(text).into_iter().map(|s| (s.what.kind, s.what.width)).collect()
    }

    #[test]
    fn indents_asked_for() {
        use IndentKind::{Spaces, Tabs};
        for (line, want) in [
            ("Indent with tabs.", vec![(Some(Tabs), None)]),
            ("Use 2 spaces for indentation.", vec![(Some(Spaces), Some(2))]),
            ("Indentation: 4-space", vec![(Some(Spaces), Some(4))]),
            ("indent width 8", vec![(None, Some(8))]),
            ("Indent with spaces, not tabs.", vec![(Some(Spaces), None)]),
            ("indent 12 spaces", vec![(Some(Spaces), None)]),
            ("indent 2 or 4 spaces", vec![(Some(Spaces), None)]),
            ("Indent with tabs or spaces.", vec![]),
            ("Open two tabs in the browser", vec![]),
            ("```\nindent with tabs\n```", vec![]),
        ] {
            assert_eq!(indent_of(line), want, "{line}");
        }
        assert_eq!(indents("x\nIndent with tabs.\n")[0].line, 2);
    }

    fn removed(text: &str, raws: &[&str], syntax: ImportSyntax, in_comments: bool) -> Option<String> {
        let raws: Vec<String> = raws.iter().map(|r| r.to_string()).collect();
        remove_imports(text, &raws, syntax, in_comments)
    }

    #[test]
    fn removing_an_import() {
        use ImportSyntax::{AgyLink, At};
        let gone = ["gone.md"];
        let some = |s: &str| Some(s.to_string());
        assert_eq!(removed("a\n@gone.md\nb\n", &gone, At, false), some("a\nb\n"));
        assert_eq!(removed("a\r\n- @gone.md\r\nb", &gone, At, false), some("a\r\nb"));
        assert_eq!(removed("a\n@gone.md", &gone, At, false), some("a\n"));
        assert_eq!(removed("see @gone.md here\n", &gone, At, false), some("see here\n"));
        assert_eq!(removed("@gone.md, then\n", &["gone.md,"], At, false), some("then\n"));
        assert_eq!(removed("see @gone.md\n", &gone, At, false), some("see\n"));
        assert_eq!(removed("x @[l](gone.md) y", &gone, AgyLink, false), some("x y"));
        assert_eq!(
            removed("\u{feff}see @gone.md here\n", &gone, At, false),
            some("\u{feff}see here\n")
        );
        assert_eq!(
            removed("@gone.md\nx\n@./gone.md\n", &["gone.md", "./gone.md"], At, false),
            some("x\n")
        );
        assert_eq!(
            removed("<!-- café --> @gone.md here\n", &gone, At, false),
            some("<!-- café --> here\n")
        );
        let commented = "<!-- @gone.md -->\n@gone.md\nx\n";
        assert_eq!(removed(commented, &gone, At, false), some("<!-- @gone.md -->\nx\n"));
        assert_eq!(removed(commented, &gone, At, true), some("<!-- -->\nx\n"));
        assert_eq!(
            removed("\u{feff}@gone.md\nrest\n", &gone, At, false),
            some("\u{feff}rest\n")
        );
        assert_eq!(removed("@other.md\n", &gone, At, false), None);
        assert_eq!(removed("see `@gone.md` here\n", &gone, At, true), None);
        assert_eq!(removed("```\n@gone.md\n```\n", &gone, At, true), None);
        assert_eq!(removed("<!-- @gone.md -->\n", &gone, At, false), None);
        assert_eq!(removed("<!-- @gone.md -->\n", &gone, At, true), some("<!-- -->\n"));
    }
}

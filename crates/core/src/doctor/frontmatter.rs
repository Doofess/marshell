//! A small reader for a `SKILL.md` frontmatter, without a YAML dependency. It reads the four fields the Doctor needs
//! and says `Unclear` whenever a line isn't one it fully follows, so a valid file never looks broken.

/// The fields of a skill's frontmatter the Doctor looks at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
    /// `disable-model-invocation: true`
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Front {
    /// Every top-level line was read.
    Read(SkillMeta),
    /// Some line wasn't understood; what was read is kept, but nothing may be called missing from it.
    Unclear(SkillMeta),
    /// No `---` first line, no closing `---`, or nothing between them.
    NoBlock,
}

/// One value, or the sign that it wasn't understood.
enum Value {
    Text(String),
    Unclear,
}

/// Reads the frontmatter of a `SKILL.md`.
pub fn read(text: &str) -> Front {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return Front::NoBlock;
    }
    let Some(close) = lines.iter().skip(1).position(|l| l.trim() == "---").map(|p| p + 1) else {
        return Front::NoBlock;
    };
    if close == 1 {
        return Front::NoBlock;
    }
    let block = &lines[1..close];

    let mut meta = SkillMeta::default();
    let mut unclear = false;
    let mut i = 0;
    while i < block.len() {
        let line = block[i];
        i += 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            // An indented line with no key above it.
            unclear = true;
            continue;
        }
        let Some((key, rest)) = line.split_once(':') else {
            unclear = true;
            continue;
        };
        let identifier = !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'));
        if !identifier || !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
            unclear = true;
            continue;
        }
        let cont = take_indented(block, &mut i);
        if !matches!(key, "name" | "description" | "when_to_use" | "disable-model-invocation") {
            continue;
        }
        match value_of(rest.trim(), &cont) {
            Value::Unclear => unclear = true,
            Value::Text(v) => match key {
                "name" => meta.name = Some(v),
                "description" => meta.description = Some(v),
                "when_to_use" => meta.when_to_use = Some(v),
                _ => meta.hidden = v.trim().eq_ignore_ascii_case("true"),
            },
        }
    }
    if unclear {
        Front::Unclear(meta)
    } else {
        Front::Read(meta)
    }
}

/// The lines after a key that are blank or indented; trailing blank lines are left out.
fn take_indented<'a>(block: &[&'a str], i: &mut usize) -> Vec<&'a str> {
    let mut out = Vec::new();
    while *i < block.len() && (block[*i].trim().is_empty() || block[*i].starts_with(char::is_whitespace)) {
        out.push(block[*i]);
        *i += 1;
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out
}

fn value_of(rest: &str, cont: &[&str]) -> Value {
    if rest.is_empty() || rest.starts_with('#') {
        return Value::Text(fold(cont));
    }
    let first = rest.chars().next().unwrap_or(' ');
    match first {
        '|' | '>' => {
            let indicator = &rest[1..];
            let ok = indicator
                .chars()
                .take_while(|c| !c.is_whitespace())
                .all(|c| matches!(c, '-' | '+' | '0'..='9'));
            let after = indicator.trim_start_matches(|c: char| !c.is_whitespace()).trim();
            if !ok || !(after.is_empty() || after.starts_with('#')) {
                return Value::Unclear;
            }
            Value::Text(block_scalar(first == '|', cont))
        }
        '"' | '\'' => quoted(rest, cont),
        '[' | '{' | '&' | '*' | '!' | '%' | '@' | '`' => Value::Unclear,
        _ => {
            let mut parts = vec![strip_comment(rest).to_string()];
            parts.extend(
                cont.iter()
                    .map(|l| l.trim())
                    .filter(|l| !l.starts_with('#'))
                    .map(|l| strip_comment(l).to_string()),
            );
            Value::Text(join_spaced(parts.iter().map(String::as_str)))
        }
    }
}

/// The continuation lines of an empty value, joined with a space.
fn fold(cont: &[&str]) -> String {
    join_spaced(cont.iter().map(|l| strip_comment(l.trim())))
}

fn join_spaced<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `#` starts a comment at the start of the text or after whitespace.
fn strip_comment(s: &str) -> &str {
    let mut prev_space = true;
    for (idx, c) in s.char_indices() {
        if c == '#' && prev_space {
            return s[..idx].trim_end();
        }
        prev_space = c.is_whitespace();
    }
    s.trim_end()
}

fn block_scalar(literal: bool, cont: &[&str]) -> String {
    let indent = cont
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);
    let lines: Vec<String> = cont
        .iter()
        .map(|l| l.chars().skip(indent).collect::<String>().trim_end().to_string())
        .collect();
    if literal {
        lines.join("\n")
    } else {
        join_spaced(lines.iter().map(String::as_str))
    }
}

fn quoted(rest: &str, cont: &[&str]) -> Value {
    let quote = rest.chars().next().unwrap_or('"');
    let (value, after, extra) = match parse_quoted(rest, quote) {
        Some((v, a)) => (v, a.to_string(), cont.iter().any(|l| !l.trim().is_empty())),
        None => {
            // A quoted value may run over several lines.
            let mut joined = rest.to_string();
            for l in cont {
                joined.push(' ');
                joined.push_str(l.trim());
            }
            match parse_quoted(&joined, quote) {
                Some((v, a)) => (v, a.to_string(), false),
                None => return Value::Unclear,
            }
        }
    };
    let after = after.trim();
    if extra || !(after.is_empty() || after.starts_with('#')) {
        return Value::Unclear;
    }
    Value::Text(value)
}

/// The text up to the closing quote (unescaped) and what follows it.
fn parse_quoted(s: &str, quote: char) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = s.char_indices().skip(1).peekable();
    while let Some((idx, c)) = chars.next() {
        if quote == '"' && c == '\\' {
            match chars.peek() {
                Some((_, n)) if *n == '"' || *n == '\\' => {
                    out.push(*n);
                    chars.next();
                }
                _ => out.push('\\'),
            }
        } else if quote == '\'' && c == '\'' {
            if matches!(chars.peek(), Some((_, '\''))) {
                out.push('\'');
                chars.next();
            } else {
                return Some((out, &s[idx + 1..]));
            }
        } else if quote == '"' && c == '"' {
            return Some((out, &s[idx + 1..]));
        } else {
            out.push(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(text: &str) -> SkillMeta {
        match read(text) {
            Front::Read(m) => m,
            other => panic!("expected Read, got {other:?}"),
        }
    }

    const PLAIN: &str = "---\nname: pdf\ndescription: Reads PDFs: text and tables\n---\nbody";

    #[test]
    fn plain_keys_are_read() {
        let m = meta(PLAIN);
        assert_eq!(m.name.as_deref(), Some("pdf"));
        assert_eq!(m.description.as_deref(), Some("Reads PDFs: text and tables"));
        assert!(!m.hidden);
    }

    #[test]
    fn block_and_folded_scalars() {
        let folded = meta("---\ndescription: >-\n  one\n  two\n---\n");
        assert_eq!(folded.description.as_deref(), Some("one two"));
        let literal = meta("---\ndescription: |\n  one\n  two\n---\n");
        assert_eq!(literal.description.as_deref(), Some("one\ntwo"));
        let keep = meta("---\ndescription: |-\n  one\n  two\nname: n\n---\n");
        assert_eq!(keep.description.as_deref(), Some("one\ntwo"));
        assert_eq!(keep.name.as_deref(), Some("n"));
    }

    #[test]
    fn quoted_values_with_hash_and_colon() {
        let m = meta("---\ndescription: \"a: b # c\"\n---\n");
        assert_eq!(m.description.as_deref(), Some("a: b # c"));
        let m = meta("---\ndescription: a # note\n---\n");
        assert_eq!(m.description.as_deref(), Some("a"));
        let m = meta("---\ndescription: 'it''s # fine'\nname: \"q\\\"x\\\\\"\n---\n");
        assert_eq!(m.description.as_deref(), Some("it's # fine"));
        assert_eq!(m.name.as_deref(), Some("q\"x\\"));
    }

    #[test]
    fn crlf_and_bom() {
        let text = format!("\u{feff}{}", PLAIN.replace('\n', "\r\n"));
        assert_eq!(read(&text), read(PLAIN));
        assert!(matches!(read(&text), Front::Read(_)));
    }

    #[test]
    fn continued_plain_value() {
        let m = meta("---\ndescription: one\n  two\nname: x\n---\n");
        assert_eq!(m.description.as_deref(), Some("one two"));
        assert_eq!(m.name.as_deref(), Some("x"));
    }

    #[test]
    fn no_block_cases() {
        assert_eq!(read("name: x\n---\n"), Front::NoBlock);
        assert_eq!(read("---\nname: x\n"), Front::NoBlock);
        assert_eq!(read("---\n---\nbody"), Front::NoBlock);
        assert_eq!(read(""), Front::NoBlock);
    }

    #[test]
    fn unclear_cases() {
        for (text, name) in [
            ("---\nname: x\ndescription: [a, b]\n---\n", Some("x")),
            ("---\nname: x\n- item\n---\n", Some("x")),
            ("---\nname: x\ndescription: \"open\n---\n", Some("x")),
        ] {
            match read(text) {
                Front::Unclear(m) => {
                    assert_eq!(m.name.as_deref(), name, "{text}");
                    assert_eq!(m.description, None, "{text}");
                }
                other => panic!("expected Unclear for {text:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn quote_can_run_over_lines_and_other_keys_are_skipped() {
        let m = meta("---\ndescription: \"one\n  two\"\nmetadata:\n  a: b\n  c: [1]\nname: n\n---\n");
        assert_eq!(m.description.as_deref(), Some("one two"));
        assert_eq!(m.name.as_deref(), Some("n"));
    }

    #[test]
    fn hidden_flag() {
        assert!(meta("---\ndisable-model-invocation: true\n---\n").hidden);
        assert!(meta("---\ndisable-model-invocation: True\n---\n").hidden);
        assert!(!meta("---\ndisable-model-invocation: false\n---\n").hidden);
    }
}

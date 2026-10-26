//! An agent CLI's user settings file, read for a view that never writes it. Everything that leaves this module has
//! been masked first; a file that doesn't parse or can't be read gives no value and no text. No value is logged.

use crate::config::jsonc::{parse_problem, read_value};
use crate::tools::secrets::{scrub, shown_words, takes_secret_words};
use marshell_protocol::settings_view::{AgentSettingsView, SettingValue, ViewScope, ViewState};
use serde_json::Value;
use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// What a masked secret shows as.
const HIDDEN: &str = "hidden";
/// A shown object or array is cut to this many characters, then `…`.
const SHOWN_CHARS: usize = 200;

/// The settings Marshell shows by name for an agent; a dotted key is looked up through nested objects.
pub fn curated(agent: &str) -> &'static [&'static str] {
    match agent {
        "claude" => &[
            "model",
            "effortLevel",
            "permissions.defaultMode",
            "cleanupPeriodDays",
            "autoUpdatesChannel",
            "theme",
        ],
        "codex" => &[
            "model",
            "model_reasoning_effort",
            "approval_policy",
            "sandbox_mode",
            "project_doc_max_bytes",
            "check_for_update_on_startup",
        ],
        "antigravity" => &[
            "model",
            "toolPermission",
            "enableTerminalSandbox",
            "colorScheme",
            "verbosity",
            "enableTelemetry",
        ],
        _ => &[],
    }
}

/// Whether a key's value is a secret: the key holds `key`, `token`, `secret`, `password` or `auth`, in any case.
pub fn is_secret_key(key: &str) -> bool {
    let key = key.to_lowercase();
    ["key", "token", "secret", "password", "auth"]
        .iter()
        .any(|word| key.contains(word))
}

// ---- masking, once for JSON and once for TOML ----

/// What the masking walk needs of a JSON or a TOML value.
trait Tree: Sized {
    fn text(s: &str) -> Self;
    fn entries(&mut self) -> Option<Vec<(&String, &mut Self)>>;
    fn items(&mut self) -> Option<&mut Vec<Self>>;
    fn string(&mut self) -> Option<&mut String>;
}

impl Tree for Value {
    fn text(s: &str) -> Self {
        Value::String(s.to_string())
    }
    fn entries(&mut self) -> Option<Vec<(&String, &mut Self)>> {
        self.as_object_mut().map(|m| m.iter_mut().collect())
    }
    fn items(&mut self) -> Option<&mut Vec<Self>> {
        self.as_array_mut()
    }
    fn string(&mut self) -> Option<&mut String> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

impl Tree for toml::Value {
    fn text(s: &str) -> Self {
        toml::Value::String(s.to_string())
    }
    fn entries(&mut self) -> Option<Vec<(&String, &mut Self)>> {
        self.as_table_mut().map(|m| m.iter_mut().collect())
    }
    fn items(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            toml::Value::Array(a) => Some(a),
            _ => None,
        }
    }
    fn string(&mut self) -> Option<&mut String> {
        match self {
            toml::Value::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Masks `value` in place: every leaf under a key named `env`, the whole value under a secret key, a secret given as
/// a flag value or `NAME=value` in an array of strings, and a secret in any other string.
pub fn mask(value: &mut Value) {
    mask_tree(value);
}

/// `mask` for a TOML value.
pub fn mask_toml(value: &mut toml::Value) {
    mask_tree(value);
}

fn mask_tree<T: Tree>(node: &mut T) {
    if let Some(entries) = node.entries() {
        for (key, child) in entries {
            if key.eq_ignore_ascii_case("env") {
                hide_leaves(child);
            } else if is_secret_key(key) {
                *child = T::text(HIDDEN);
            } else {
                mask_tree(child);
            }
        }
    } else if let Some(items) = node.items() {
        let all_strings = !items.is_empty() && items.iter_mut().all(|i| i.string().is_some());
        if all_strings {
            let words: Vec<String> = items.iter_mut().filter_map(|i| i.string().cloned()).collect();
            *items = mask_words(&words).iter().map(|w| T::text(w)).collect();
        } else {
            items.iter_mut().for_each(mask_tree);
        }
    } else if let Some(s) = node.string() {
        *s = mask_string(s).into_owned();
    }
}

/// Every leaf under `node` becomes `hidden`; the keys stay.
fn hide_leaves<T: Tree>(node: &mut T) {
    if let Some(entries) = node.entries() {
        entries.into_iter().for_each(|(_, child)| hide_leaves(child));
    } else if let Some(items) = node.items() {
        items.iter_mut().for_each(hide_leaves);
    } else {
        *node = T::text(HIDDEN);
    }
}

/// An array of strings (an argument list): a command that takes a secret shows as its program and subcommands; a
/// value after a secret-named flag is hidden; the rest goes through `mask_string`.
fn mask_words(words: &[String]) -> Vec<String> {
    let mut out = words.to_vec();
    let mut at = 0;
    while at < out.len() {
        let flag = out[at].strip_prefix('-').map(|f| f.trim_start_matches('-'));
        if flag.is_some_and(|f| !f.contains('=') && is_secret_key(f)) && at + 1 < out.len() {
            out[at + 1] = HIDDEN.to_string();
            at += 1;
        }
        at += 1;
    }
    let out: Vec<String> = out.iter().map(|w| mask_string(w).into_owned()).collect();
    // The values are hidden by now, so what `shown_words` prints can't hold one.
    if takes_secret_words(&out) {
        return vec![mask_string(&shown_words(&out)).into_owned()];
    }
    out
}

/// A string that holds a secret: `NAME=value` pieces with a secret name, `Name: value` with a secret name, and a
/// command line that takes a secret flag (`scrub`).
fn mask_string(s: &str) -> Cow<'_, str> {
    match hide_assignments(s) {
        Cow::Owned(assigned) => Cow::Owned(scrub(&assigned).into_owned()),
        Cow::Borrowed(same) => scrub(same),
    }
}

/// A name that is a secret key, possibly with leading dashes (`--api-key`, `API_KEY`).
fn is_secret_name(name: &str) -> bool {
    let bare = name.trim_start_matches('-');
    !bare.is_empty()
        && bare
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && is_secret_key(bare)
}

/// Hides, anywhere in `s`: the password of URL userinfo, the value after a secret-named `Name:` (to the end of the
/// quoted string it sits in, or of `s`), and the value of a secret-named `NAME=value`.
fn hide_assignments(s: &str) -> Cow<'_, str> {
    let out = hide_assignment_values(&hide_headers(&hide_url_credentials(s)));
    if out == s { Cow::Borrowed(s) } else { Cow::Owned(out) }
}

/// `scheme://user:password@host` shows as `scheme://user:hidden@host`; a userinfo with no password is hidden whole.
fn hide_url_credentials(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\'' | '\\'))
            .unwrap_or(tail.len());
        let (authority, after) = tail.split_at(end);
        match authority.rfind('@') {
            Some(i) => {
                let (userinfo, host) = authority.split_at(i);
                if let Some((user, _)) = userinfo.split_once(':') {
                    out.push_str(user);
                    out.push(':');
                }
                out.push_str(HIDDEN);
                out.push_str(host);
            }
            None => out.push_str(authority),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The byte offset of the first `quote` in `text` that no backslash escapes.
fn find_unescaped(text: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == quote {
            return Some(i);
        }
    }
    None
}

/// `Name: value` with a secret name (a leading quote or dashes allowed, one closing quote before the colon) hides the
/// value up to the closing quote of the string the colon sits in, or the end of `s` when it isn't quoted.
fn hide_headers(s: &str) -> String {
    let is_name_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.');
    let mut out = String::with_capacity(s.len());
    // Everything before `copied` is in `out` or hidden.
    let mut copied = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if i < copied {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' | '\'' => match quote {
                None => quote = Some(c),
                Some(q) if q == c => quote = None,
                Some(_) => {}
            },
            ':' => {
                let before = s[..i].strip_suffix(['"', '\'']).unwrap_or(&s[..i]);
                let name_at = before
                    .char_indices()
                    .rev()
                    .find(|(_, c)| !is_name_char(*c))
                    .map_or(0, |(k, c)| k + c.len_utf8());
                let value = &s[i + 1..];
                if is_secret_name(&before[name_at..]) && !value.trim().is_empty() {
                    let end = quote
                        .and_then(|q| find_unescaped(value, q))
                        .map_or(s.len(), |k| i + 1 + k);
                    out.push_str(&s[copied..=i]);
                    out.push(' ');
                    out.push_str(HIDDEN);
                    copied = end;
                }
            }
            _ => {}
        }
    }
    out.push_str(&s[copied.min(s.len())..]);
    out
}

fn is_assignment_separator(c: char) -> bool {
    c.is_whitespace() || c == '&' || c == '?'
}

/// The first word of `text`, up to a space, `&` or `?`.
fn first_word(text: &str) -> &str {
    &text[..text.find(is_assignment_separator).unwrap_or(text.len())]
}

/// Where the value that starts at byte `at` of `s` ends: a quoted value at its closing quote, an auth scheme word
/// (`Bearer`, `Basic`, `Token`) after the word that follows it, any other value at the next space, `&` or `?`.
fn value_end(s: &str, at: usize) -> usize {
    let value = &s[at..];
    let unslashed = value.trim_start_matches('\\');
    if let Some(q @ ('"' | '\'')) = unslashed.chars().next() {
        let opened = at + (value.len() - unslashed.len()) + 1;
        return s[opened..].find(q).map_or(s.len(), |k| opened + k + 1);
    }
    let word_end = at + first_word(value).len();
    let is_scheme = ["bearer", "basic", "token"]
        .iter()
        .any(|w| first_word(value).eq_ignore_ascii_case(w));
    if is_scheme && s[word_end..].starts_with(char::is_whitespace) {
        let next = s.len() - s[word_end..].trim_start_matches(char::is_whitespace).len();
        return next + first_word(&s[next..]).len();
    }
    word_end
}

/// `NAME=value` with a secret name hides the value, however `value_end` finds its end.
fn hide_assignment_values(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    while at < s.len() {
        let token_end = at + first_word(&s[at..]).len();
        let token = &s[at..token_end];
        let lead = token.len() - token.trim_start_matches(['"', '\'']).len();
        let hidden_name = token[lead..]
            .split_once('=')
            .filter(|(name, value)| !value.is_empty() && *value != HIDDEN && is_secret_name(name));
        let mut end = token_end;
        if let Some((name, _)) = hidden_name {
            let value_at = at + lead + name.len() + 1;
            out.push_str(&s[at..value_at]);
            out.push_str(HIDDEN);
            end = value_end(s, value_at);
        } else {
            out.push_str(token);
        }
        // The separator that ended it stays.
        match s[end..].chars().next() {
            Some(c) if is_assignment_separator(c) => {
                out.push(c);
                at = end + c.len_utf8();
            }
            _ => at = end,
        }
    }
    out
}

// ---- the view ----

/// The first of the managed files that reads and parses (highest precedence first).
pub fn read_managed(paths: &[PathBuf]) -> Option<Value> {
    paths.iter().find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        if parse_problem(&text).is_some() {
            return None;
        }
        read_value(&text).ok()
    })
}

fn toml_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(a) => Value::Array(a.iter().map(toml_json).collect()),
        toml::Value::Table(t) => Value::Object(t.iter().map(|(k, v)| (k.clone(), toml_json(v))).collect()),
    }
}

/// A masked value as shown: a string as written, a number or bool as JSON, an object or array as compact JSON cut at
/// 200 characters.
fn shown(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(_) | Value::Array(_) => {
            let text = value.to_string();
            if text.chars().count() > SHOWN_CHARS {
                let mut cut: String = text.chars().take(SHOWN_CHARS).collect();
                cut.push('…');
                cut
            } else {
                text
            }
        }
        other => other.to_string(),
    }
}

fn lookup<'a>(root: &'a Value, dotted: &str) -> Option<&'a Value> {
    dotted.split('.').try_fold(root, |node, part| node.get(part))
}

/// The 1-based line of byte offset `at` in `text`.
fn line_at(text: &str, at: usize) -> u32 {
    let end = at.min(text.len());
    let breaks = text.as_bytes()[..end].iter().filter(|b| **b == b'\n').count();
    u32::try_from(breaks + 1).unwrap_or(u32::MAX)
}

/// The view of `agent`'s settings file at `path`, given what reading it gave. `managed` is Claude's managed settings
/// (already read); it is ignored for any other agent.
pub fn view(
    agent: &str,
    path: &Path,
    text: Result<String, std::io::Error>,
    managed: Option<Value>,
) -> AgentSettingsView {
    let mut out = AgentSettingsView {
        agent: agent.to_string(),
        path: path.to_string_lossy().into_owned(),
        state: ViewState::Ok,
        curated: Vec::new(),
        others: Vec::new(),
        raw: None,
    };
    let mut managed = managed.filter(|_| agent == "claude");
    if let Some(m) = managed.as_mut() {
        mask(m);
    }
    let (user, raw) = match text {
        Ok(text) if agent == "codex" => match text.strip_prefix('\u{feff}').unwrap_or(&text).parse::<toml::Table>() {
            Ok(table) => {
                let mut value = toml::Value::Table(table);
                mask_toml(&mut value);
                (Some(toml_json(&value)), toml::to_string_pretty(&value).ok())
            }
            Err(e) => {
                out.state = ViewState::Unparsable {
                    line: e.span().map_or(1, |span| line_at(&text, span.start)),
                };
                return out;
            }
        },
        Ok(text) => {
            let parsed = match parse_problem(&text) {
                Some((line, _)) => Err(line),
                None => read_value(&text).map_err(|_| 1),
            };
            match parsed {
                Ok(mut value) => {
                    mask(&mut value);
                    let raw = serde_json::to_string_pretty(&value).ok();
                    (Some(value), raw)
                }
                Err(line) => {
                    out.state = ViewState::Unparsable { line };
                    return out;
                }
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            out.state = ViewState::Missing;
            (None, None)
        }
        Err(_) => {
            out.state = ViewState::Unreadable;
            return out;
        }
    };
    out.raw = raw;
    let value_of = |key: &str| -> SettingValue {
        let from_managed = managed.as_ref().and_then(|m| lookup(m, key)).map(shown);
        let (value, scope) = match from_managed {
            Some(v) => (Some(v), ViewScope::Managed),
            None => (user.as_ref().and_then(|u| lookup(u, key)).map(shown), ViewScope::User),
        };
        SettingValue {
            key: key.to_string(),
            value,
            scope,
        }
    };
    out.curated = curated(agent).iter().map(|key| value_of(key)).collect();
    if let Some(Value::Object(map)) = &user {
        let mut keys: Vec<&String> = map.keys().filter(|k| !curated(agent).contains(&k.as_str())).collect();
        keys.sort();
        out.others = keys
            .into_iter()
            .map(|k| SettingValue {
                key: k.clone(),
                value: Some(shown(&map[k])),
                scope: ViewScope::User,
            })
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Error, ErrorKind};

    fn run(agent: &str, text: &str) -> AgentSettingsView {
        view(agent, Path::new("settings"), Ok(text.to_string()), None)
    }

    fn value_of<'a>(list: &'a [SettingValue], key: &str) -> Option<&'a SettingValue> {
        list.iter().find(|s| s.key == key)
    }

    #[test]
    fn masks_env_and_secret_keys() {
        let mut v = json!({
            "env": {"ANTHROPIC_API_KEY": "sk-ant-x", "DEBUG": "1"},
            "apiKeyHelper": "op read x",
            "model": "opus",
            "mcp": {"s": {"headers": {"Authorization": "Bearer y"}}},
            "list": [{"token": "z"}]
        });
        mask(&mut v);
        assert_eq!(v["env"]["ANTHROPIC_API_KEY"], "hidden");
        assert_eq!(v["env"]["DEBUG"], "hidden");
        assert_eq!(v["apiKeyHelper"], "hidden");
        assert_eq!(v["mcp"]["s"]["headers"]["Authorization"], "hidden");
        assert_eq!(v["list"][0]["token"], "hidden");
        assert_eq!(v["model"], "opus");
        let all = v.to_string();
        for gone in ["sk-ant-x", "op read x", "Bearer y", "\"z\""] {
            assert!(!all.contains(gone), "{gone} in {all}");
        }
    }

    #[test]
    fn env_is_found_at_any_depth_and_in_any_case() {
        let mut v = json!({"a": {"b": [{"ENV": {"X": 1, "Y": [true, "p"]}}]}});
        mask(&mut v);
        assert_eq!(v["a"]["b"][0]["ENV"], json!({"X": "hidden", "Y": ["hidden", "hidden"]}));
    }

    #[test]
    fn a_secret_flag_value_in_an_array_is_hidden() {
        let mut v = json!({"mcpServers": {"s": {"command": "npx", "args": ["-y", "srv", "--api-key", "sk-abc"]}}});
        mask(&mut v);
        assert!(!v.to_string().contains("sk-abc"), "{v}");
        let mut v = json!({"args": ["--token", "sk-abc", "--password=sk-def", "API_KEY=sk-ghi"]});
        mask(&mut v);
        let all = v.to_string();
        for gone in ["sk-abc", "sk-def", "sk-ghi"] {
            assert!(!all.contains(gone), "{gone} in {all}");
        }
        let mut v = json!({"args": ["docker", "login", "-u", "me", "-p", "sk-jkl"]});
        mask(&mut v);
        assert!(!v.to_string().contains("sk-jkl"), "{v}");
    }

    #[test]
    fn a_secret_in_a_command_string_is_hidden() {
        let mut v = json!({"statusLine": {"type": "command", "command": "run --token abc123"}, "model": "opus"});
        mask(&mut v);
        assert!(!v.to_string().contains("abc123"), "{v}");
        assert_eq!(v["model"], "opus");
        let mut v = json!({"url": "https://x.example/mcp?api_key=abc123&v=2", "h": "Authorization: Bearer abc123"});
        mask(&mut v);
        assert!(!v.to_string().contains("abc123"), "{v}");
        assert!(v["url"].as_str().unwrap().contains("v=2"));
    }

    #[test]
    fn codex_toml_masks_nested_env() {
        let text = "model = \"gpt-5\"\n[mcp_servers.x]\ncommand = \"node\"\nargs = [\"--token\", \"abc\"]\n[mcp_servers.x.env]\nAPI_KEY = \"abc\"\n";
        let v = run("codex", text);
        assert_eq!(v.state, ViewState::Ok);
        assert_eq!(value_of(&v.curated, "model").unwrap().value.as_deref(), Some("gpt-5"));
        let servers = value_of(&v.others, "mcp_servers").unwrap().value.clone().unwrap();
        assert!(servers.contains("hidden") && !servers.contains("abc"), "{servers}");
        let raw = v.raw.unwrap();
        assert!(raw.contains("hidden") && !raw.contains("abc"), "{raw}");
    }

    #[test]
    fn codex_toml_masks_inline_tables_and_arrays_of_tables() {
        let text = "[[servers]]\nname = \"a\"\nheaders = { Authorization = \"Bearer abc\" }\n\n[[servers]]\nenv = { K = \"abc\" }\n";
        let v = run("codex", text);
        let all = format!("{:?}", v);
        assert!(!all.contains("abc"), "{all}");
        assert!(v.raw.unwrap().contains("hidden"));
    }

    #[test]
    fn unparsable_returns_nothing() {
        for (agent, text) in [("claude", "{\"model\": \"opus\","), ("codex", "model = ")] {
            let v = run(agent, text);
            assert!(matches!(v.state, ViewState::Unparsable { .. }), "{agent}");
            assert!(
                v.curated.is_empty() && v.others.is_empty() && v.raw.is_none(),
                "{agent}"
            );
        }
        assert_eq!(
            run("claude", "{\"model\": \"opus\",").state,
            ViewState::Unparsable { line: 1 }
        );
        assert_eq!(
            run("claude", "{\n\"model\": \"opus\",\n\"x\" 1}").state,
            ViewState::Unparsable { line: 3 }
        );
        assert_eq!(
            run("codex", "a = 1\nmodel = \n").state,
            ViewState::Unparsable { line: 2 }
        );
    }

    #[test]
    fn missing_has_curated_dashes() {
        let v = view(
            "claude",
            Path::new("s.json"),
            Err(Error::from(ErrorKind::NotFound)),
            None,
        );
        assert_eq!(v.state, ViewState::Missing);
        assert_eq!(v.curated.len(), 6);
        assert!(
            v.curated
                .iter()
                .all(|s| s.value.is_none() && s.scope == ViewScope::User)
        );
        assert!(v.others.is_empty() && v.raw.is_none());
    }

    #[test]
    fn unreadable() {
        let v = view(
            "codex",
            Path::new("c.toml"),
            Err(Error::from(ErrorKind::PermissionDenied)),
            None,
        );
        assert_eq!(v.state, ViewState::Unreadable);
        assert!(v.curated.is_empty() && v.others.is_empty() && v.raw.is_none());
    }

    #[test]
    fn dotted_curated_and_others() {
        let v = run(
            "claude",
            "{\"permissions\":{\"defaultMode\":\"plan\",\"deny\":[\"x\"]},\"zeta\":1,\"alpha\":true}",
        );
        assert_eq!(
            value_of(&v.curated, "permissions.defaultMode")
                .unwrap()
                .value
                .as_deref(),
            Some("plan")
        );
        let keys: Vec<&str> = v.others.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["alpha", "permissions", "zeta"]);
        assert_eq!(v.others[0].value.as_deref(), Some("true"));
        assert_eq!(v.others[2].value.as_deref(), Some("1"));
    }

    #[test]
    fn a_curated_top_level_key_is_not_in_others() {
        let v = run("claude", "{\"model\":\"opus\",\"theme\":\"dark\"}");
        assert!(v.others.is_empty());
        assert_eq!(value_of(&v.curated, "theme").unwrap().value.as_deref(), Some("dark"));
    }

    #[test]
    fn long_values_are_cut() {
        let items: Vec<String> = (0..100).map(|i| format!("item-{i}")).collect();
        let v = run("claude", &json!({"list": items}).to_string());
        let shown = v.others[0].value.clone().unwrap();
        assert!(shown.ends_with('…'));
        assert!(shown.chars().count() <= 201);
    }

    #[test]
    fn managed_wins_for_curated() {
        let v = view(
            "claude",
            Path::new("s.json"),
            Ok("{\"permissions\":{\"defaultMode\":\"plan\"}}".into()),
            Some(json!({"permissions": {"defaultMode": "default"}})),
        );
        let m = value_of(&v.curated, "permissions.defaultMode").unwrap();
        assert_eq!(m.value.as_deref(), Some("default"));
        assert_eq!(m.scope, ViewScope::Managed);
        assert_eq!(value_of(&v.curated, "model").unwrap().scope, ViewScope::User);
    }

    #[test]
    fn managed_is_masked_and_ignored_for_other_agents() {
        let v = view(
            "claude",
            Path::new("s.json"),
            Err(Error::from(ErrorKind::NotFound)),
            Some(json!({"theme": "dark", "env": {"A": "b"}})),
        );
        assert_eq!(value_of(&v.curated, "theme").unwrap().scope, ViewScope::Managed);
        let v = view(
            "codex",
            Path::new("c.toml"),
            Ok("model = \"gpt-5\"".into()),
            Some(json!({"model": "x"})),
        );
        let m = value_of(&v.curated, "model").unwrap();
        assert_eq!((m.value.as_deref(), m.scope), (Some("gpt-5"), ViewScope::User));
    }

    #[test]
    fn is_secret_key_cases() {
        for k in [
            "apiKeyHelper",
            "GITHUB_TOKEN",
            "Authorization",
            "password",
            "client_secret",
        ] {
            assert!(is_secret_key(k), "{k}");
        }
        for k in ["model", "theme", "cleanupPeriodDays"] {
            assert!(!is_secret_key(k), "{k}");
        }
    }

    #[test]
    fn a_secret_top_level_key_shows_hidden_and_the_raw_text_holds_no_secret() {
        let v = run("claude", "{\"apiKeyHelper\":\"op read x\",\"env\":{\"K\":\"sk-1\"}}");
        assert_eq!(
            value_of(&v.others, "apiKeyHelper").unwrap().value.as_deref(),
            Some("hidden")
        );
        let all = format!("{v:?}");
        assert!(!all.contains("sk-1") && !all.contains("op read"), "{all}");
    }

    fn masked(s: &str) -> String {
        mask_string(s).into_owned()
    }

    #[test]
    fn a_secret_header_inside_a_command_is_hidden() {
        assert_eq!(
            masked("curl -H 'Authorization: Bearer sk-live-1' https://x"),
            "curl -H 'Authorization: hidden' https://x"
        );
        assert_eq!(
            masked("curl -H \"X-Api-Key: sk-live-2\" https://x"),
            "curl -H \"X-Api-Key: hidden\" https://x"
        );
        assert_eq!(
            masked("bash -c \"curl -H \\\"Authorization: Bearer sk-live-3\\\" https://x\""),
            "bash -c \"curl -H \\\"Authorization: hidden\""
        );
        // Not quoted: hidden to the end of the string.
        let out = masked("curl --header Authorization: Bearer sk-live-4 https://x");
        assert!(
            !out.contains("sk-live-4") && out.contains("Authorization: hidden"),
            "{out}"
        );
        assert_eq!(masked("Authorization: Bearer abc123"), "Authorization: hidden");
        assert_eq!(masked("--token: sk-live-5"), "--token: hidden");
        assert_eq!(
            masked("Content-Type: application/json"),
            "Content-Type: application/json"
        );
    }

    #[test]
    fn a_scheme_value_after_a_secret_assignment_is_hidden() {
        assert_eq!(
            masked("AUTH_HEADER=Bearer sk-x node s.js"),
            "AUTH_HEADER=hidden node s.js"
        );
        assert_eq!(
            masked("AUTH_HEADER=basic sk-x node s.js"),
            "AUTH_HEADER=hidden node s.js"
        );
        assert_eq!(masked("API_TOKEN=Token sk-x run"), "API_TOKEN=hidden run");
        assert_eq!(masked("TOKEN=\"a b c\" run"), "TOKEN=hidden run");
        assert_eq!(masked("TOKEN='a b c' run"), "TOKEN=hidden run");
        assert_eq!(masked("TOKEN=\\\"a b c\\\" run"), "TOKEN=hidden run");
        assert_eq!(masked("FOO=bar API_KEY=k1 node"), "FOO=bar API_KEY=hidden node");
        assert_eq!(masked("TOKEN=sk-1 run"), "TOKEN=hidden run");
        assert_eq!(masked("NOTE=bearer sk-y"), "NOTE=bearer sk-y");
    }

    #[test]
    fn url_credentials_are_hidden() {
        assert_eq!(masked("postgresql://u:pw@h/db"), "postgresql://u:hidden@h/db");
        assert_eq!(
            masked("run postgres://u:pw@h:5432/db?sslmode=require now"),
            "run postgres://u:hidden@h:5432/db?sslmode=require now"
        );
        assert_eq!(masked("https://ghp_tok@github.com/x"), "https://hidden@github.com/x");
        assert_eq!(masked("https://example.com/a"), "https://example.com/a");
        assert_eq!(masked("postgresql://u:hidden@h/db"), "postgresql://u:hidden@h/db");
        let mut v = json!({"mcp": {"url": "https://u:pw@h/x"}, "env2": ["see postgresql://u:pw@h/db"]});
        mask(&mut v);
        assert!(!v.to_string().contains("pw@"), "{v}");
    }

    #[test]
    fn a_url_with_credentials_in_a_codex_args_array_is_hidden() {
        let text = "[mcp_servers.pg]
command = \"npx\"
args = [\"-y\",\"@modelcontextprotocol/server-postgres\",\"postgresql://u:pw@h/db\"]
";
        let v = run("codex", text);
        let all = format!("{v:?}");
        assert!(!all.contains("pw@"), "{all}");
        assert!(v.raw.unwrap().contains("u:hidden@h/db"));
    }

    #[test]
    fn a_hooks_command_with_a_secret_header_leaks_nothing_through_the_view() {
        let text = json!({
            "hooks": {"PostToolUse": [{"hooks": [{"type": "command",
                "command": "curl -H 'Authorization: Bearer sk-live-1' https://x && AUTH_HEADER=Bearer sk-x node s.js"}]}]},
            "statusLine": {"type": "command", "command": "TOKEN=\"a b c\" run postgresql://u:pw@h/db"}
        })
        .to_string();
        for agent in ["claude"] {
            let v = run(agent, &text);
            let all = format!("{v:?}");
            for gone in ["sk-live-1", "sk-x", "a b c", "u:pw@"] {
                assert!(!all.contains(gone), "{gone} in {all}");
            }
            assert!(v.raw.unwrap().contains("Authorization: hidden"));
            assert!(value_of(&v.others, "hooks").is_some());
        }
    }
}

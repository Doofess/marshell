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

fn hide_assignments(s: &str) -> Cow<'_, str> {
    if let Some((name, rest)) = s.split_once(':')
        && !rest.trim().is_empty()
        && is_secret_name(name)
    {
        return Cow::Owned(format!("{name}: {HIDDEN}"));
    }
    let is_sep = |c: char| c.is_whitespace() || c == '&' || c == '?';
    let mut out = String::with_capacity(s.len());
    let mut changed = false;
    for piece in s.split_inclusive(is_sep) {
        let (body, sep) = match piece.char_indices().last() {
            Some((at, c)) if is_sep(c) => piece.split_at(at),
            _ => (piece, ""),
        };
        match body.split_once('=') {
            Some((name, value)) if !value.is_empty() && value != HIDDEN && is_secret_name(name) => {
                out.push_str(name);
                out.push('=');
                out.push_str(HIDDEN);
                changed = true;
            }
            _ => out.push_str(body),
        }
        out.push_str(sep);
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(s) }
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
}

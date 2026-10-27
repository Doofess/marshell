//! Key edits on TOML text, made with `toml_edit` so comments, blank lines and key order survive
//! (PLAN §6). If the text doesn't parse, the edit fails and nothing is written.

use crate::config::jsonc::EditError;
use serde_json::Value;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value as TomlValue};

const BOM: char = '\u{feff}';

/// One edit of a TOML document, addressed by a key path (one part per table level).
pub enum TomlOp<'a> {
    Set { key: &'a [String], value: &'a Value },
    Remove { key: &'a [String] },
}

/// Applies the ops in order and returns the new text, or the old text untouched when nothing changed.
/// `text` may start with a BOM and may be empty. A value must be a string, an integer, a float or a bool.
pub fn apply_toml_ops(text: &str, ops: &[TomlOp]) -> Result<String, EditError> {
    let (had_bom, body) = match text.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut doc: DocumentMut = body
        .parse()
        .map_err(|e: toml_edit::TomlError| EditError::DoesNotParse(e.to_string()))?;
    let mut changed = false;
    for op in ops {
        changed |= match op {
            TomlOp::Set { key, value } => set(doc.as_table_mut(), key, value)?,
            TomlOp::Remove { key } => remove(doc.as_table_mut(), key),
        };
    }
    if !changed {
        return Ok(text.to_string());
    }
    let mut out = doc.to_string();
    if out.parse::<DocumentMut>().is_err() {
        return Err(EditError::Internal("the edited text does not parse".to_string()));
    }
    if had_bom {
        out.insert(0, BOM);
    }
    Ok(out)
}

fn scalar(value: &Value) -> Result<TomlValue, EditError> {
    match value {
        Value::String(s) => basic_string(s),
        Value::Bool(b) => Ok((*b).into()),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Ok(i.into()),
            (None, Some(f)) if n.is_f64() => Ok(f.into()),
            _ => Err(EditError::Internal("the number does not fit a TOML value".to_string())),
        },
        _ => Err(EditError::Internal(
            "a TOML value must be a string, an integer, a float or a bool".to_string(),
        )),
    }
}

/// A string as a basic (double-quoted) TOML string. `toml_edit` would pick a literal string for text that holds
/// quotes, so the quoted form is written out and read back.
fn basic_string(text: &str) -> Result<TomlValue, EditError> {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
        .parse()
        .map_err(|e: toml_edit::TomlError| EditError::Internal(e.to_string()))
}

fn same(a: &TomlValue, b: &TomlValue) -> bool {
    match (a, b) {
        (TomlValue::String(a), TomlValue::String(b)) => a.value() == b.value(),
        (TomlValue::Integer(a), TomlValue::Integer(b)) => a.value() == b.value(),
        (TomlValue::Float(a), TomlValue::Float(b)) => a.value() == b.value(),
        (TomlValue::Boolean(a), TomlValue::Boolean(b)) => a.value() == b.value(),
        _ => false,
    }
}

fn not_a_table(key: &[String], detail: &str) -> EditError {
    EditError::BadPointer {
        pointer: key.join("."),
        detail: detail.to_string(),
    }
}

/// Sets `key` and says whether the text changes. A parent table that is missing is created. An existing value
/// is replaced in place with its decor, so a trailing comment stays.
fn set(root: &mut Table, key: &[String], value: &Value) -> Result<bool, EditError> {
    let Some((last, parents)) = key.split_last() else {
        return Err(EditError::Internal("the key is empty".to_string()));
    };
    let mut new = scalar(value)?;
    let mut changed = false;
    let mut table: &mut dyn TableLike = root;
    for (depth, part) in parents.iter().enumerate() {
        if !table.contains_key(part) {
            let mut created = Table::new();
            created.set_implicit(true);
            table.insert(part, Item::Table(created));
            changed = true;
        }
        let Some(item) = table.get_mut(part) else {
            return Err(EditError::Internal("a table vanished while editing".to_string()));
        };
        table = item
            .as_table_like_mut()
            .ok_or_else(|| not_a_table(&key[..=depth], "is not a table"))?;
    }
    match table.get_mut(last) {
        Some(item) => {
            let Some(old) = item.as_value_mut() else {
                return Err(not_a_table(key, "is a table, not a value"));
            };
            if same(old, &new) {
                return Ok(changed);
            }
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        None => {
            table.insert(last, Item::Value(new));
        }
    }
    Ok(true)
}

/// Removes `key` and its line; says whether anything was there.
fn remove(root: &mut Table, key: &[String]) -> bool {
    let Some((last, parents)) = key.split_last() else {
        return false;
    };
    let mut table: &mut dyn TableLike = root;
    for part in parents {
        match table.get_mut(part).and_then(Item::as_table_like_mut) {
            Some(next) => table = next,
            None => return false,
        }
    }
    table.remove(last).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::jsonc::EditError;
    use serde_json::{Value, json};

    fn key(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|p| p.to_string()).collect()
    }

    fn set(text: &str, parts: &[&str], value: Value) -> Result<String, EditError> {
        let key = key(parts);
        apply_toml_ops(
            text,
            &[TomlOp::Set {
                key: &key,
                value: &value,
            }],
        )
    }

    fn remove(text: &str, parts: &[&str]) -> Result<String, EditError> {
        let key = key(parts);
        apply_toml_ops(text, &[TomlOp::Remove { key: &key }])
    }

    #[test]
    fn set_keeps_comments_blank_lines_and_order() {
        let text = "# mine\nmodel = \"a\" # why\n\n[profiles.x]\nmodel = \"b\"\n";
        assert_eq!(
            set(text, &["model"], json!("c")).unwrap(),
            "# mine\nmodel = \"c\" # why\n\n[profiles.x]\nmodel = \"b\"\n"
        );
    }

    #[test]
    fn set_adds_a_missing_top_level_key_before_the_first_table() {
        let text = "[profiles.x]\nmodel = \"b\"\n";
        let out = set(text, &["sandbox_mode"], json!("read-only")).unwrap();
        let parsed: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(parsed["sandbox_mode"].as_str(), Some("read-only"));
        assert_eq!(parsed["profiles"]["x"]["model"].as_str(), Some("b"));
        assert!(out.ends_with("[profiles.x]\nmodel = \"b\"\n"), "{out}");
    }

    #[test]
    fn set_adds_after_the_last_top_level_key() {
        let text = "a = 1\n\n[t]\nb = 2\n";
        let out = set(text, &["c"], json!(3)).unwrap();
        assert_eq!(out, "a = 1\nc = 3\n\n[t]\nb = 2\n");
    }

    #[test]
    fn set_creates_the_tables_a_key_runs_through() {
        let out = set("a = 1\n", &["profiles", "x", "model"], json!("m")).unwrap();
        let parsed: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(parsed["profiles"]["x"]["model"].as_str(), Some("m"));
        assert!(out.contains("[profiles.x]"), "{out}");
    }

    #[test]
    fn set_on_an_empty_file() {
        assert_eq!(
            set("", &["check_for_update_on_startup"], json!(false)).unwrap(),
            "check_for_update_on_startup = false\n"
        );
    }

    #[test]
    fn setting_the_same_value_leaves_the_text_alone() {
        let text = "model = 'a' # why\n";
        assert_eq!(set(text, &["model"], json!("a")).unwrap(), text);
    }

    #[test]
    fn remove_takes_the_key_and_its_line() {
        assert_eq!(remove("model = \"a\"\nx = 1\n", &["model"]).unwrap(), "x = 1\n");
        let text = "x = 1\n";
        assert_eq!(remove(text, &["model"]).unwrap(), text);
        assert_eq!(remove(text, &["model", "deeper"]).unwrap(), text);
    }

    #[test]
    fn values_keep_their_type() {
        assert_eq!(set("", &["n"], json!(32768)).unwrap(), "n = 32768\n");
        assert_eq!(set("", &["b"], json!(true)).unwrap(), "b = true\n");
        assert_eq!(set("", &["f"], json!(1.5)).unwrap(), "f = 1.5\n");
        let out = set("", &["s"], json!("say \"hi\"")).unwrap();
        assert_eq!(out, "s = \"say \\\"hi\\\"\"\n");
        let parsed: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(parsed["s"].as_str(), Some("say \"hi\""));
    }

    #[test]
    fn a_table_or_array_value_is_refused() {
        assert!(set("", &["a"], json!({"b": 1})).is_err());
        assert!(set("", &["a"], json!([1, 2])).is_err());
        assert!(set("", &["a"], Value::Null).is_err());
    }

    #[test]
    fn a_key_through_a_non_table_is_bad_pointer() {
        let err = set("model = \"a\"\n", &["model", "x"], json!(1)).unwrap_err();
        assert!(matches!(err, EditError::BadPointer { .. }), "{err:?}");
    }

    #[test]
    fn an_empty_key_is_refused() {
        assert!(set("", &[], json!(1)).is_err());
    }

    #[test]
    fn text_that_does_not_parse_is_refused() {
        let err = set("model = \n", &["x"], json!(1)).unwrap_err();
        assert!(matches!(err, EditError::DoesNotParse(_)), "{err:?}");
        assert!(matches!(
            remove("model = \n", &["x"]).unwrap_err(),
            EditError::DoesNotParse(_)
        ));
    }
}

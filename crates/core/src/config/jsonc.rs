//! JSON pointer edits on JSONC text, made through the CST so comments, key order, line endings
//! and a BOM survive (PLAN §6). There is no whole-file re-serialize fallback: if the CST can't
//! parse a file, or an edit does not read back as the ops say, the edit fails and nothing is
//! written.

use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstArray, CstContainerNode, CstInputValue, CstLeafNode, CstNode, CstObject, CstRootNode};
use serde_json::Value;

const BOM: char = '\u{feff}';

/// One edit of a JSON document, addressed by an RFC 6901 pointer.
pub enum JsonOp<'a> {
    Set { pointer: &'a str, value: &'a Value },
    Remove { pointer: &'a str },
}

#[derive(Debug, PartialEq)]
pub enum EditError {
    DoesNotParse(String),
    BadPointer { pointer: String, detail: String },
    Internal(String),
}

/// Applies the ops in order and returns the new text. `text` may start with a BOM and may be
/// empty or whitespace only (treated as `{}`).
pub fn apply_ops(text: &str, ops: &[JsonOp]) -> Result<String, EditError> {
    let (had_bom, body) = match text.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let source = if body.trim().is_empty() { "{}" } else { body };
    let root =
        CstRootNode::parse(source, &ParseOptions::default()).map_err(|e| EditError::DoesNotParse(e.to_string()))?;

    // What the ops should produce, computed on plain values.
    let mut expected = parse_value(source)?;
    let mut changed = false;
    for op in ops {
        changed |= apply_to_value(&mut expected, op)?;
        changed |= apply_to_cst(&root, op)?;
    }
    if !changed {
        return Ok(text.to_string());
    }

    let mut out = root.to_string();
    if body.contains("\r\n") && !has_lone_lf(body) {
        out = lf_to_crlf(&out);
    }
    if had_bom {
        out.insert(0, BOM);
    }

    match read_value(&out) {
        Ok(actual) if actual == expected => Ok(out),
        Ok(_) => Err(EditError::Internal(
            "the edited text does not read back as the requested change".to_string(),
        )),
        Err(e) => Err(EditError::Internal(format!("the edited text does not parse: {e:?}"))),
    }
}

/// Reads JSONC (comments, trailing commas) into a serde value, for reading only. Empty or
/// whitespace-only text reads as `{}`, as `apply_ops` treats it.
pub fn read_value(text: &str) -> Result<Value, EditError> {
    let body = text.strip_prefix(BOM).unwrap_or(text);
    if body.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    parse_value(body)
}

/// Where `text` stops being JSONC: the 1-based line of the first parse error and what is wrong.
/// `None` when it parses (empty text counts, as in `read_value`).
pub fn parse_problem(text: &str) -> Option<(u32, String)> {
    let body = text.strip_prefix(BOM).unwrap_or(text);
    if body.trim().is_empty() {
        return None;
    }
    jsonc_parser::parse_to_serde_value::<Value>(body, &ParseOptions::default())
        .err()
        .map(|e| {
            (
                u32::try_from(e.line_display()).unwrap_or(u32::MAX),
                e.kind().to_string(),
            )
        })
}

fn parse_value(body: &str) -> Result<Value, EditError> {
    jsonc_parser::parse_to_serde_value::<Value>(body, &ParseOptions::default())
        .map_err(|e| EditError::DoesNotParse(e.to_string()))
}

fn has_lone_lf(s: &str) -> bool {
    let b = s.as_bytes();
    b.iter()
        .enumerate()
        .any(|(i, &c)| c == b'\n' && (i == 0 || b[i - 1] != b'\r'))
}

fn lf_to_crlf(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 16);
    let mut prev = '\0';
    for c in s.chars() {
        if c == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(c);
        prev = c;
    }
    out
}

fn bad(pointer: &str, detail: &str) -> EditError {
    EditError::BadPointer {
        pointer: pointer.to_string(),
        detail: detail.to_string(),
    }
}

fn parse_pointer(pointer: &str) -> Result<Vec<String>, EditError> {
    if pointer.is_empty() {
        return Err(bad(pointer, "the root cannot be set or removed"));
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Err(bad(pointer, "a pointer starts with /"));
    };
    rest.split('/')
        .map(|raw| {
            let mut out = String::with_capacity(raw.len());
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => out.push('~'),
                        Some('1') => out.push('/'),
                        _ => return Err(bad(pointer, "~ must be followed by 0 or 1")),
                    }
                } else {
                    out.push(c);
                }
            }
            Ok(out)
        })
        .collect()
}

/// An array index token: digits only, no leading zero unless it is `0` itself.
fn array_index(token: &str) -> Option<usize> {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    token.parse().ok()
}

// ---- the reference implementation, on plain values ----

/// Applies the op to a value. Returns whether anything changed.
fn apply_to_value(root: &mut Value, op: &JsonOp) -> Result<bool, EditError> {
    match op {
        JsonOp::Set { pointer, value } => {
            let tokens = parse_pointer(pointer)?;
            if !root.is_object() {
                return Err(bad(pointer, "the root must be an object"));
            }
            let (last, parents) = tokens.split_last().expect("a pointer has a token");
            let mut cur = root;
            for t in parents {
                cur = match cur {
                    Value::Object(map) => map
                        .entry(t.clone())
                        .or_insert_with(|| Value::Object(Default::default())),
                    Value::Array(items) => {
                        let i = array_index(t)
                            .filter(|i| *i < items.len())
                            .ok_or_else(|| bad(pointer, "no such array element"))?;
                        &mut items[i]
                    }
                    _ => return Err(bad(pointer, "the path goes through a scalar")),
                };
            }
            match cur {
                Value::Object(map) => {
                    map.insert(last.clone(), (*value).clone());
                }
                Value::Array(items) => {
                    if last == "-" {
                        items.push((*value).clone());
                    } else {
                        let i = array_index(last)
                            .filter(|i| *i < items.len())
                            .ok_or_else(|| bad(pointer, "no such array element"))?;
                        items[i] = (*value).clone();
                    }
                }
                _ => return Err(bad(pointer, "the path goes through a scalar")),
            }
            Ok(true)
        }
        JsonOp::Remove { pointer } => {
            let tokens = parse_pointer(pointer)?;
            if !root.is_object() {
                return Err(bad(pointer, "the root must be an object"));
            }
            let (last, parents) = tokens.split_last().expect("a pointer has a token");
            let mut cur = root;
            for t in parents {
                cur = match cur {
                    Value::Object(map) => match map.get_mut(t) {
                        Some(v) => v,
                        None => return Ok(false),
                    },
                    Value::Array(items) => match array_index(t).and_then(|i| items.get_mut(i)) {
                        Some(v) => v,
                        None => return Ok(false),
                    },
                    _ => return Ok(false),
                };
            }
            match cur {
                Value::Object(map) => Ok(map.shift_remove(last).is_some()),
                Value::Array(items) => match array_index(last).filter(|i| *i < items.len()) {
                    Some(i) => {
                        items.remove(i);
                        Ok(true)
                    }
                    None => Ok(false),
                },
                _ => Ok(false),
            }
        }
    }
}

// ---- the CST edits ----

fn to_input(v: &Value) -> CstInputValue {
    match v {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(to_input).collect()),
        Value::Object(map) => CstInputValue::Object(map.iter().map(|(k, v)| (k.clone(), to_input(v))).collect()),
    }
}

/// `tokens` as nested objects around `leaf`: `[a, b]` and 1 gives `{"a": {"b": 1}}`.
fn nest(tokens: &[String], leaf: CstInputValue) -> CstInputValue {
    tokens
        .iter()
        .rev()
        .fold(leaf, |inner, t| CstInputValue::Object(vec![(t.clone(), inner)]))
}

fn replace_node(node: CstNode, value: CstInputValue) -> bool {
    match node {
        CstNode::Container(CstContainerNode::Object(n)) => n.replace_with(value).is_some(),
        CstNode::Container(CstContainerNode::Array(n)) => n.replace_with(value).is_some(),
        CstNode::Container(_) => false,
        CstNode::Leaf(CstLeafNode::StringLit(n)) => n.replace_with(value).is_some(),
        CstNode::Leaf(CstLeafNode::NumberLit(n)) => n.replace_with(value).is_some(),
        CstNode::Leaf(CstLeafNode::BooleanLit(n)) => n.replace_with(value).is_some(),
        CstNode::Leaf(CstLeafNode::NullKeyword(n)) => n.replace_with(value).is_some(),
        CstNode::Leaf(CstLeafNode::WordLit(n)) => n.replace_with(value).is_some(),
        CstNode::Leaf(_) => false,
    }
}

enum Cur {
    Obj(CstObject),
    Arr(CstArray),
}

fn cur_of(node: CstNode) -> Option<Cur> {
    node.as_object().map(Cur::Obj).or_else(|| node.as_array().map(Cur::Arr))
}

fn root_object(root: &CstRootNode, pointer: &str) -> Result<CstObject, EditError> {
    match root.value() {
        None => Ok(root.object_value_or_set()),
        Some(node) => node
            .as_object()
            .ok_or_else(|| bad(pointer, "the root must be an object")),
    }
}

/// Applies the op to the CST. Returns whether anything changed.
fn apply_to_cst(root: &CstRootNode, op: &JsonOp) -> Result<bool, EditError> {
    let pointer = match op {
        JsonOp::Set { pointer, .. } | JsonOp::Remove { pointer } => *pointer,
    };
    let tokens = parse_pointer(pointer)?;
    let (last, parents) = tokens.split_last().expect("a pointer has a token");
    let mut cur = Cur::Obj(root_object(root, pointer)?);

    for (n, t) in parents.iter().enumerate() {
        let next = match &cur {
            Cur::Obj(obj) => match obj.get(t) {
                Some(prop) => prop.value(),
                None => {
                    let JsonOp::Set { value, .. } = op else {
                        return Ok(false);
                    };
                    // Create the rest of the path in one insert.
                    obj.append(t, nest(&tokens[n + 1..], to_input(value)));
                    return Ok(true);
                }
            },
            Cur::Arr(arr) => array_index(t).and_then(|i| arr.elements().into_iter().nth(i)),
        };
        match next.and_then(cur_of) {
            Some(c) => cur = c,
            None => {
                return match op {
                    JsonOp::Set { .. } => Err(bad(pointer, "the path goes through a scalar")),
                    JsonOp::Remove { .. } => Ok(false),
                };
            }
        }
    }

    match (op, cur) {
        (JsonOp::Set { value, .. }, Cur::Obj(obj)) => {
            match obj.get(last) {
                Some(prop) => prop.set_value(to_input(value)),
                None => {
                    obj.append(last, to_input(value));
                }
            }
            Ok(true)
        }
        (JsonOp::Set { value, .. }, Cur::Arr(arr)) => {
            if last == "-" {
                arr.append(to_input(value));
                return Ok(true);
            }
            let node = array_index(last)
                .and_then(|i| arr.elements().into_iter().nth(i))
                .ok_or_else(|| bad(pointer, "no such array element"))?;
            if replace_node(node, to_input(value)) {
                Ok(true)
            } else {
                Err(EditError::Internal("could not replace an array element".to_string()))
            }
        }
        (JsonOp::Remove { .. }, Cur::Obj(obj)) => match obj.get(last) {
            Some(prop) => {
                prop.remove();
                Ok(true)
            }
            None => Ok(false),
        },
        (JsonOp::Remove { .. }, Cur::Arr(arr)) => {
            match array_index(last).and_then(|i| arr.elements().into_iter().nth(i)) {
                Some(node) => {
                    node.remove();
                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMENTED: &str =
        "{\n  // my theme\n  \"theme\": \"dark\",\n  \"zeta\": 1, /* trailing */\n  \"alpha\": 2,\n}\n";

    #[test]
    fn set_keeps_comments_and_key_order() {
        let v = serde_json::json!([{"type": "command"}]);
        let out = apply_ops(
            COMMENTED,
            &[JsonOp::Set {
                pointer: "/hooks/Stop",
                value: &v,
            }],
        )
        .unwrap();
        assert!(out.contains("// my theme") && out.contains("/* trailing */"));
        let pos = |k: &str| out.find(k).unwrap();
        assert!(
            pos("\"theme\"") < pos("\"zeta\"")
                && pos("\"zeta\"") < pos("\"alpha\"")
                && pos("\"alpha\"") < pos("\"hooks\"")
        );
        assert_eq!(read_value(&out).unwrap()["hooks"]["Stop"], v);
    }

    #[test]
    fn set_replaces_an_existing_value_in_place() {
        let out = apply_ops(
            "{\"a\": 1, \"b\": 2}",
            &[JsonOp::Set {
                pointer: "/a",
                value: &5.into(),
            }],
        )
        .unwrap();
        assert_eq!(out, "{\"a\": 5, \"b\": 2}");
    }

    #[test]
    fn set_creates_missing_parents() {
        let v = serde_json::json!(true);
        let out = apply_ops(
            "{}",
            &[JsonOp::Set {
                pointer: "/a/b/c",
                value: &v,
            }],
        )
        .unwrap();
        assert_eq!(read_value(&out).unwrap(), serde_json::json!({"a": {"b": {"c": true}}}));
    }

    #[test]
    fn append_to_array_with_dash() {
        let v = serde_json::json!({"y": 2});
        let out = apply_ops(
            "{\"hooks\":{\"Stop\":[{\"x\":1}]}}",
            &[JsonOp::Set {
                pointer: "/hooks/Stop/-",
                value: &v,
            }],
        )
        .unwrap();
        let stop = read_value(&out).unwrap()["hooks"]["Stop"].clone();
        assert_eq!(stop.as_array().unwrap().len(), 2);
        assert_eq!(stop[1], v);
    }

    #[test]
    fn remove_drops_the_entry_and_its_comma() {
        let out = apply_ops("{\"a\": 1, \"b\": 2, \"c\": 3}", &[JsonOp::Remove { pointer: "/b" }]).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&out).unwrap(),
            serde_json::json!({"a": 1, "c": 3})
        );
    }

    #[test]
    fn remove_missing_pointer_is_no_change() {
        let out = apply_ops(COMMENTED, &[JsonOp::Remove { pointer: "/nope/x" }]).unwrap();
        assert_eq!(out, COMMENTED);
    }

    #[test]
    fn pointer_escapes() {
        let one = serde_json::json!(1);
        let two = serde_json::json!(2);
        let out = apply_ops(
            "{}",
            &[
                JsonOp::Set {
                    pointer: "/a~1b",
                    value: &one,
                },
                JsonOp::Set {
                    pointer: "/m~0n",
                    value: &two,
                },
            ],
        )
        .unwrap();
        assert_eq!(read_value(&out).unwrap(), serde_json::json!({"a/b": 1, "m~n": 2}));
    }

    #[test]
    fn unparseable_file_fails_closed() {
        assert!(matches!(
            apply_ops("{ \"a\": ", &[JsonOp::Remove { pointer: "/a" }]),
            Err(EditError::DoesNotParse(_))
        ));
    }

    #[test]
    fn set_through_a_scalar_is_bad_pointer() {
        let v = serde_json::json!(1);
        assert!(matches!(
            apply_ops(
                "{\"a\":1}",
                &[JsonOp::Set {
                    pointer: "/a/b",
                    value: &v
                }]
            ),
            Err(EditError::BadPointer { .. })
        ));
    }

    #[test]
    fn root_must_be_an_object() {
        let v = serde_json::json!(1);
        assert!(matches!(
            apply_ops(
                "[1]",
                &[JsonOp::Set {
                    pointer: "/a",
                    value: &v
                }]
            ),
            Err(EditError::BadPointer { .. })
        ));
        assert!(matches!(
            apply_ops("{}", &[JsonOp::Set { pointer: "", value: &v }]),
            Err(EditError::BadPointer { .. })
        ));
    }

    #[test]
    fn crlf_stays_crlf() {
        let out = apply_ops(
            "{\r\n  \"a\": 1\r\n}\r\n",
            &[JsonOp::Set {
                pointer: "/b",
                value: &serde_json::json!({"c": 2}),
            }],
        )
        .unwrap();
        assert!(
            out.match_indices('\n')
                .all(|(i, _)| i > 0 && out.as_bytes()[i - 1] == b'\r'),
            "{out:?}"
        );
    }

    #[test]
    fn bom_is_kept() {
        let out = apply_ops(
            "\u{feff}{\"a\": 1}",
            &[JsonOp::Set {
                pointer: "/b",
                value: &2.into(),
            }],
        )
        .unwrap();
        assert!(out.starts_with('\u{feff}'));
        assert_eq!(read_value(&out).unwrap(), serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn parse_problem_names_the_line() {
        assert_eq!(parse_problem("{}\n"), None);
        assert_eq!(parse_problem("  \n"), None);
        assert_eq!(parse_problem(COMMENTED), None);
        let (line, message) = parse_problem("{\n  \"a\": 1,\n  \"b\": }\n").unwrap();
        assert_eq!(line, 3);
        assert!(!message.is_empty());
        // A BOM and CRLF do not shift the line.
        let (line, _) = parse_problem("\u{feff}{\r\n  \"a\": 1,\r\n  \"b\": }\r\n").unwrap();
        assert_eq!(line, 3);
    }

    #[test]
    fn whitespace_only_file_starts_as_an_object() {
        let out = apply_ops(
            "  \n",
            &[JsonOp::Set {
                pointer: "/a",
                value: &1.into(),
            }],
        )
        .unwrap();
        assert_eq!(read_value(&out).unwrap(), serde_json::json!({"a": 1}));
    }
}

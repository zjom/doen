//! Format-preserving YAML frontmatter.
//!
//! The frontmatter is split into blocks, one for each top-level key. A block keeps its
//! original text. doen replaces the text of a block only when doen changes the value of the
//! key. Thus, comments, unknown fields and the format of other fields stay as they are.
//!
//! doen reads every scalar as a string. YAML type rules never change a value.

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;

/// The fields that doen manages, in the order that doen writes them.
pub const MANAGED: [&str; 11] = [
    "id",
    "kind",
    "created",
    "date",
    "scheduled",
    "due",
    "priority",
    "tags",
    "archived",
    "trashed",
    "status",
];

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Seq(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Str(s) => serde_json::Value::String(s.clone()),
            Value::Seq(v) => serde_json::Value::Array(v.iter().map(Value::to_json).collect()),
            Value::Map(m) => {
                serde_json::Value::Object(m.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Block {
    key: String,
    /// Comment lines and blank lines before the key.
    lead: String,
    /// The key line and its continuation lines.
    body: String,
    value: Value,
}

#[derive(Clone, Debug)]
pub struct Doc {
    open: String,
    blocks: Vec<Block>,
    tail: String,
    close: String,
}

/// Splits a file into the frontmatter text and the body. Returns `None` if the file has no
/// frontmatter.
pub fn split_file(text: &str) -> Option<(&str, &str, &str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first_end = text.find('\n')? + 1;
    if text[..first_end].trim_end() != "---" {
        return None;
    }
    let mut pos = first_end;
    while pos < text.len() {
        let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i + 1);
        let line = text[pos..end].trim_end();
        if line == "---" || line == "..." {
            return Some((
                &text[..first_end],
                &text[first_end..pos],
                &text[pos..end],
                &text[end..],
            ));
        }
        pos = end;
    }
    None
}

impl Doc {
    /// Parses a whole file. Returns the frontmatter and the body, or `None` if the file has
    /// no frontmatter.
    pub fn parse_file(text: &str) -> Option<Result<(Doc, String), String>> {
        let (open, fm, close, body) = split_file(text)?;
        Some(Doc::parse(fm).map(|mut d| {
            d.open = open.to_string();
            d.close = if close.ends_with('\n') {
                close.to_string()
            } else {
                format!("{close}\n")
            };
            (d, body.to_string())
        }))
    }

    pub fn empty() -> Doc {
        Doc {
            open: "---\n".into(),
            blocks: vec![],
            tail: String::new(),
            close: "---\n".into(),
        }
    }

    /// Parses frontmatter text (without the `---` lines).
    pub fn parse(fm: &str) -> Result<Doc, String> {
        let mut doc = Doc::empty();
        let mut pending = String::new();
        let mut current: Option<(String, String)> = None;
        for line in fm.split_inclusive('\n') {
            let line = if line.ends_with('\n') {
                line.to_string()
            } else {
                format!("{line}\n")
            };
            let first = line.chars().next().unwrap_or('\n');
            let trimmed = line.trim();
            let is_trivia = trimmed.is_empty() || (first == '#');
            let is_key = !is_trivia && !first.is_whitespace() && first != '-';
            if is_key {
                if let Some((lead, body)) = current.take() {
                    doc.push_block(lead, body)?;
                }
                current = Some((std::mem::take(&mut pending), line));
            } else if is_trivia {
                pending.push_str(&line);
            } else if let Some((_, body)) = current.as_mut() {
                body.push_str(&pending);
                pending.clear();
                body.push_str(&line);
            } else {
                return Err(format!("unexpected content: '{}'", trimmed));
            }
        }
        if let Some((lead, body)) = current {
            doc.push_block(lead, body)?;
        }
        doc.tail = pending;
        Ok(doc)
    }

    fn push_block(&mut self, lead: String, body: String) -> Result<(), String> {
        let (key, value) = parse_block(&body)?;
        if self.get(&key).is_some() {
            return Err(format!("duplicate key '{key}'"));
        }
        self.blocks.push(Block {
            key,
            lead,
            body,
            value,
        });
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.blocks.iter().find(|b| b.key == key).map(|b| &b.value)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().map(|b| b.key.as_str())
    }

    /// Sets a field. `quote` puts every scalar of the value in quotation marks.
    pub fn set(&mut self, key: &str, value: Value, quote: bool) {
        if self.get(key) == Some(&value) {
            return;
        }
        let body = render_field(key, &value, quote);
        self.set_raw(key, value, body);
    }

    fn set_raw(&mut self, key: &str, value: Value, body: String) {
        if let Some(b) = self.blocks.iter_mut().find(|b| b.key == key) {
            b.body = body;
            b.value = value;
            return;
        }
        let block = Block {
            key: key.into(),
            lead: String::new(),
            body,
            value,
        };
        let at = match MANAGED.iter().position(|k| *k == key) {
            Some(rank) => {
                let rank_of = |b: &Block| MANAGED.iter().position(|k| *k == b.key);
                self.blocks
                    .iter()
                    .position(|b| rank_of(b).is_some_and(|r| r > rank))
                    .or_else(|| {
                        self.blocks
                            .iter()
                            .rposition(|b| rank_of(b).is_some())
                            .map(|i| i + 1)
                    })
                    .unwrap_or(0)
            }
            None => self.blocks.len(),
        };
        self.blocks.insert(at, block);
    }

    /// Copies a field from another document with its original text.
    pub fn copy_from(&mut self, other: &Doc, key: &str) {
        if let Some(b) = other.blocks.iter().find(|b| b.key == key) {
            self.set_raw(key, b.value.clone(), b.body.clone());
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.blocks.retain(|b| b.key != key);
    }

    /// The frontmatter with the `---` lines.
    pub fn render(&self) -> String {
        let mut s = self.open.clone();
        for b in &self.blocks {
            s.push_str(&b.lead);
            s.push_str(&b.body);
        }
        s.push_str(&self.tail);
        s.push_str(&self.close);
        s
    }

    pub fn to_json(&self) -> serde_json::Map<String, serde_json::Value> {
        self.blocks
            .iter()
            .map(|b| (b.key.clone(), b.value.to_json()))
            .collect()
    }
}

pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_plain_safe(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:+/".contains(c))
}

fn scalar(s: &str, quote_it: bool) -> String {
    if quote_it || !is_plain_safe(s) {
        quote(s)
    } else {
        s.to_string()
    }
}

fn render_field(key: &str, value: &Value, q: bool) -> String {
    let mut out = format!("{}:", scalar(key, false));
    render_value(&mut out, value, q, 0);
    out.push('\n');
    out
}

fn render_value(out: &mut String, value: &Value, q: bool, indent: usize) {
    let pad = " ".repeat(indent);
    match value {
        Value::Str(s) => {
            out.push(' ');
            out.push_str(&scalar(s, q));
        }
        Value::Seq(items) if items.iter().all(|v| matches!(v, Value::Str(_))) => {
            let parts: Vec<String> = items
                .iter()
                .map(|v| scalar(v.as_str().unwrap_or(""), q))
                .collect();
            out.push_str(&format!(" [{}]", parts.join(", ")));
        }
        Value::Seq(items) => {
            for item in items {
                out.push_str(&format!("\n{pad}  -"));
                match item {
                    Value::Map(m) => {
                        for (i, (k, v)) in m.iter().enumerate() {
                            if i > 0 {
                                out.push_str(&format!("\n{pad}    "));
                            } else {
                                out.push(' ');
                            }
                            out.push_str(&format!("{}:", scalar(k, false)));
                            render_value(out, v, q, indent + 4);
                        }
                    }
                    v => render_value(out, v, q, indent + 2),
                }
            }
        }
        Value::Map(m) if m.is_empty() => out.push_str(" {}"),
        Value::Map(m) => {
            for (k, v) in m {
                out.push_str(&format!("\n{pad}  {}:", scalar(k, false)));
                render_value(out, v, q, indent + 2);
            }
        }
    }
}

/// Builds a string-only tree from parser events.
#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    root: Option<Value>,
    error: Option<String>,
}

enum Frame {
    Seq(Vec<Value>),
    Map(Vec<(String, Value)>, Option<String>),
}

impl Builder {
    fn push_value(&mut self, v: Value) {
        match self.stack.last_mut() {
            None => {
                if self.root.is_some() {
                    self.error = Some("more than one document".into());
                }
                self.root = Some(v);
            }
            Some(Frame::Seq(items)) => items.push(v),
            Some(Frame::Map(entries, key)) => match key.take() {
                None => match v {
                    Value::Str(k) => *key = Some(k),
                    _ => self.error = Some("complex keys are not supported".into()),
                },
                Some(k) => entries.push((k, v)),
            },
        }
    }
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, ev: Event, _mark: Marker) {
        match ev {
            Event::Scalar(s, ..) => self.push_value(Value::Str(s)),
            Event::SequenceStart(..) => self.stack.push(Frame::Seq(vec![])),
            Event::MappingStart(..) => self.stack.push(Frame::Map(vec![], None)),
            Event::SequenceEnd | Event::MappingEnd => match self.stack.pop() {
                Some(Frame::Seq(items)) => self.push_value(Value::Seq(items)),
                Some(Frame::Map(entries, _)) => self.push_value(Value::Map(entries)),
                None => {}
            },
            Event::Alias(_) => self.error = Some("aliases are not supported".into()),
            _ => {}
        }
    }
}

pub fn parse_yaml(text: &str) -> Result<Option<Value>, String> {
    let mut b = Builder::default();
    Parser::new_from_str(text)
        .load(&mut b, true)
        .map_err(|e| e.to_string())?;
    match b.error {
        Some(e) => Err(e),
        None => Ok(b.root),
    }
}

fn parse_block(body: &str) -> Result<(String, Value), String> {
    match parse_yaml(body)? {
        Some(Value::Map(mut m)) if m.len() == 1 => Ok(m.pop().unwrap()),
        _ => Err(format!(
            "invalid field: '{}'",
            body.lines().next().unwrap_or("").trim()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "---\nid: \"k3f9\"\n# a comment\nkind: todo\ncreated: 2026-10-16T21:58:03+11:00\nmine: no\nnum: 1e10\nzero: 0012\ntags: [\"a\"]\nlist:\n- x\n- y   # trailing\nstatus:\n  - date: 2026-10-05\n    status: drop\n# tail\n---\n# [ ] call bank\nbody\n";

    #[test]
    fn round_trip_and_strings() {
        let (doc, body) = Doc::parse_file(FILE).unwrap().unwrap();
        assert_eq!(format!("{}{}", doc.render(), body), FILE);
        assert_eq!(doc.get_str("id"), Some("k3f9"));
        assert_eq!(doc.get_str("mine"), Some("no"));
        assert_eq!(doc.get_str("num"), Some("1e10"));
        assert_eq!(doc.get_str("zero"), Some("0012"));
        assert_eq!(doc.get_str("created"), Some("2026-10-16T21:58:03+11:00"));
        assert_eq!(
            doc.get("list"),
            Some(&Value::Seq(vec![
                Value::Str("x".into()),
                Value::Str("y".into())
            ]))
        );
        let st = doc.get("status").unwrap();
        let Value::Seq(entries) = st else { panic!() };
        assert_eq!(entries[0].get("status"), Some(&Value::Str("drop".into())));
    }

    #[test]
    fn set_keeps_other_blocks() {
        let (mut doc, _) = Doc::parse_file(FILE).unwrap().unwrap();
        doc.set("due", Value::Str("2026-10-18".into()), false);
        doc.set(
            "tags",
            Value::Seq(vec![Value::Str("a".into()), Value::Str("no".into())]),
            true,
        );
        let out = doc.render();
        assert!(out.contains("# a comment\nkind: todo\n"));
        assert!(out.contains("tags: [\"a\", \"no\"]\n"));
        assert!(out.contains("mine: no\nnum: 1e10\n"));
        // `due` goes before `tags`.
        assert!(out.find("due: 2026-10-18").unwrap() < out.find("tags:").unwrap());
        doc.set(
            "status",
            Value::Seq(vec![Value::Map(vec![
                ("date".into(), Value::Str("2026-10-05".into())),
                ("status".into(), Value::Str("done".into())),
            ])]),
            false,
        );
        assert!(
            doc.render()
                .contains("status:\n  - date: 2026-10-05\n    status: done\n# tail\n---\n")
        );
        doc.remove("mine");
        assert!(!doc.render().contains("mine"));
    }

    #[test]
    fn invalid() {
        assert!(Doc::parse("a: [1\n").is_err());
        assert!(Doc::parse("a: 1\na: 2\n").is_err());
        assert!(Doc::parse("  - x\n").is_err());
        assert!(Doc::parse_file("no frontmatter\n").is_none());
    }
}

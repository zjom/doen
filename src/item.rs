//! The item model: a note or a todo, parsed from one Markdown file.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate};

use crate::dates::{format_iso, parse_iso, parse_timestamp};
use crate::frontmatter::{Doc, Value};
use crate::tags;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    #[value(alias = "n")]
    Note,
    #[value(alias = "t")]
    Todo,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Note => "note",
            Kind::Todo => "todo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Open,
    Done,
    Drop,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Done => "done",
            Status::Drop => "drop",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "open" => Some(Status::Open),
            "done" => Some(Status::Done),
            "drop" => Some(Status::Drop),
            _ => None,
        }
    }

    pub fn checkbox(self) -> char {
        match self {
            Status::Open => ' ',
            Status::Done => 'x',
            Status::Drop => '-',
        }
    }

    pub fn from_checkbox(c: char) -> Status {
        match c {
            'x' => Status::Done,
            '-' => Status::Drop,
            _ => Status::Open,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Place {
    Root,
    Archive,
    Trash,
}

impl Place {
    pub fn as_str(self) -> &'static str {
        match self {
            Place::Root => "root",
            Place::Archive => "archive",
            Place::Trash => "trash",
        }
    }

    pub fn dir(self) -> &'static str {
        match self {
            Place::Root => "",
            Place::Archive => ".archive",
            Place::Trash => ".trash",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    /// The byte range of the heading line in the body, without the newline.
    pub start: usize,
    pub end: usize,
    /// The checkbox character (`' '`, `'x'` or `'-'`). Todos only.
    pub checkbox: Option<char>,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct Meta {
    pub id: String,
    pub kind: Kind,
    pub created: DateTime<FixedOffset>,
    pub date: NaiveDate,
    pub due: Option<NaiveDate>,
    pub tags: Vec<String>,
    /// Tags in the file that are not valid.
    pub bad_tags: Vec<String>,
    pub archived: Option<NaiveDate>,
    pub trashed: Option<NaiveDate>,
    pub status: Vec<(NaiveDate, Status)>,
    pub heading: Option<Heading>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub place: Place,
    pub doc: Doc,
    pub body: String,
    pub meta: Meta,
}

/// Finds the H1 heading: the first line of the body that is not blank.
pub fn find_heading(body: &str, kind: Kind) -> Option<Heading> {
    let mut pos = 0;
    for line in body.split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let text = line.trim_end_matches(['\n', '\r']);
        if text.trim().is_empty() {
            continue;
        }
        let rest = text.strip_prefix('#')?;
        if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
            return None;
        }
        let mut title = rest.trim().trim_end_matches(" #").trim_end().to_string();
        let mut checkbox = None;
        if kind == Kind::Todo {
            let b = title.as_bytes();
            if b.len() >= 3
                && b[0] == b'['
                && b[2] == b']'
                && matches!(b[1], b' ' | b'x' | b'X' | b'-')
                && (b.len() == 3 || b[3] == b' ' || b[3] == b'\t')
            {
                checkbox = Some((b[1] as char).to_ascii_lowercase());
                title = title[3..].trim().to_string();
            }
        }
        return Some(Heading {
            start,
            end: start + text.len(),
            checkbox,
            title,
        });
    }
    None
}

fn date_field(doc: &Doc, key: &str) -> Result<Option<NaiveDate>, String> {
    match doc.get(key) {
        None => Ok(None),
        Some(Value::Str(s)) => parse_iso(s)
            .map(Some)
            .ok_or_else(|| format!("invalid date in '{key}': '{s}'")),
        Some(_) => Err(format!("'{key}' must be a date")),
    }
}

impl Meta {
    /// Reads the metadata. Returns `Ok(None)` if the frontmatter has no `id` field.
    pub fn parse(doc: &Doc, body: &str) -> Result<Option<Meta>, String> {
        let Some(id) = doc.get("id") else {
            return Ok(None);
        };
        let id = id.as_str().ok_or("'id' must be a string")?.to_string();
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return Err(format!("invalid id '{id}'"));
        }
        let kind = match doc.get_str("kind") {
            Some("note") => Kind::Note,
            Some("todo") => Kind::Todo,
            Some(k) => return Err(format!("invalid kind '{k}'")),
            None => return Err("missing 'kind'".into()),
        };
        let created = match doc.get_str("created") {
            Some(s) => parse_timestamp(s)
                .ok_or_else(|| format!("invalid timestamp in 'created': '{s}'"))?,
            None => return Err("missing 'created'".into()),
        };
        let date = date_field(doc, "date")?.ok_or("missing 'date'")?;
        let due = date_field(doc, "due")?;
        if due.is_some() && kind == Kind::Note {
            return Err("a note cannot have 'due'".into());
        }
        let mut tag_list = vec![];
        let mut bad_tags = vec![];
        match doc.get("tags") {
            None => {}
            Some(Value::Seq(items)) => {
                for t in items {
                    let t = t.as_str().ok_or("'tags' must be a list of strings")?;
                    match tags::normalize(t) {
                        Ok(t) => tags::merge(&mut tag_list, [t]),
                        Err(_) => bad_tags.push(t.to_string()),
                    }
                }
            }
            Some(_) => return Err("'tags' must be a list".into()),
        }
        let mut status = vec![];
        match doc.get("status") {
            None => {}
            Some(Value::Seq(entries)) => {
                for e in entries {
                    let d = e.get("date").and_then(Value::as_str).and_then(parse_iso);
                    let s = e
                        .get("status")
                        .and_then(Value::as_str)
                        .and_then(Status::parse);
                    match (d, s) {
                        (Some(d), Some(s)) => status.push((d, s)),
                        _ => return Err("invalid entry in 'status'".into()),
                    }
                }
                if kind == Kind::Note {
                    return Err("a note cannot have 'status'".into());
                }
            }
            Some(_) => return Err("'status' must be a list".into()),
        }
        Ok(Some(Meta {
            id,
            kind,
            created,
            date,
            due,
            tags: tag_list,
            bad_tags,
            archived: date_field(doc, "archived")?,
            trashed: date_field(doc, "trashed")?,
            status,
            heading: find_heading(body, kind),
        }))
    }

    pub fn current_status(&self) -> Option<Status> {
        match self.kind {
            Kind::Note => None,
            Kind::Todo => Some(self.status.last().map_or(Status::Open, |e| e.1)),
        }
    }

    /// The date of the last status entry, if the todo is closed.
    pub fn closed_on(&self) -> Option<NaiveDate> {
        match self.status.last() {
            Some((d, s)) if *s != Status::Open => Some(*d),
            _ => None,
        }
    }

    pub fn expected_place(&self) -> Place {
        if self.trashed.is_some() {
            Place::Trash
        } else if self.archived.is_some() {
            Place::Archive
        } else {
            Place::Root
        }
    }
}

impl Item {
    /// Parses a file. Returns `Ok(None)` if the file is not a doen item.
    pub fn parse(path: &Path, place: Place, text: &str) -> Result<Option<Item>, String> {
        let Some(parsed) = Doc::parse_file(text) else {
            return Ok(None);
        };
        let (doc, body) = parsed?;
        if doc.get("id").is_none() {
            return Ok(None);
        }
        Item::new(path.to_path_buf(), place, doc, body).map(Some)
    }

    pub fn new(path: PathBuf, place: Place, doc: Doc, body: String) -> Result<Item, String> {
        let meta = Meta::parse(&doc, &body)?.ok_or("missing 'id'")?;
        Ok(Item {
            path,
            place,
            doc,
            body,
            meta,
        })
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    pub fn kind(&self) -> Kind {
        self.meta.kind
    }

    pub fn title(&self) -> &str {
        self.meta
            .heading
            .as_ref()
            .map_or("(untitled)", |h| h.title.as_str())
    }

    pub fn has_title(&self) -> bool {
        self.meta
            .heading
            .as_ref()
            .is_some_and(|h| !h.title.is_empty())
    }

    pub fn label(&self) -> String {
        format!("{}  {}", self.id(), self.title())
    }

    pub fn render(&self) -> String {
        format!("{}{}", self.doc.render(), self.body)
    }

    /// Re-reads the metadata after a change to the frontmatter or the body.
    pub fn refresh(&mut self) -> Result<(), String> {
        self.meta = Meta::parse(&self.doc, &self.body)?.ok_or("missing 'id'")?;
        Ok(())
    }

    fn refreshed(&mut self) {
        self.refresh().expect("doen wrote invalid frontmatter");
    }

    pub fn set_date(&mut self, key: &str, d: Option<NaiveDate>) {
        match d {
            Some(d) => self.doc.set(key, Value::Str(format_iso(d)), false),
            None => self.doc.remove(key),
        }
        self.refreshed();
    }

    pub fn set_tags(&mut self, tags: &[String]) {
        self.doc.set("tags", tags::to_value(tags), true);
        self.refreshed();
    }

    pub fn set_id(&mut self, id: &str) {
        self.doc.set("id", Value::Str(id.into()), true);
        self.refreshed();
    }

    /// Appends an entry to the status log and rewrites the checkbox.
    pub fn push_status(&mut self, date: NaiveDate, status: Status) {
        let mut entries = match self.doc.get("status") {
            Some(Value::Seq(e)) => e.clone(),
            _ => vec![],
        };
        entries.push(Value::Map(vec![
            ("date".into(), Value::Str(format_iso(date))),
            ("status".into(), Value::Str(status.as_str().into())),
        ]));
        self.doc.set("status", Value::Seq(entries), false);
        self.refreshed();
        self.sync_checkbox();
    }

    /// Rewrites the heading line with a checkbox and a title.
    pub fn set_heading(&mut self, checkbox: Option<char>, title: &str) {
        let line = match checkbox {
            Some(c) => format!("# [{c}] {title}"),
            None => format!("# {title}"),
        };
        let line = line.trim_end().to_string();
        match &self.meta.heading {
            Some(h) => self.body.replace_range(h.start..h.end, &line),
            None => self.body.insert_str(0, &format!("{line}\n")),
        }
        self.refreshed();
    }

    /// The checkbox that the status log asks for, if it differs from the heading.
    pub fn checkbox_mismatch(&self) -> Option<char> {
        let want = self.meta.current_status()?.checkbox();
        let h = self.meta.heading.as_ref()?;
        (h.checkbox != Some(want)).then_some(want)
    }

    /// Rewrites the checkbox from the status log. Returns `true` if the body changed.
    pub fn sync_checkbox(&mut self) -> bool {
        match self.checkbox_mismatch() {
            Some(c) => {
                let title = self.title().to_string();
                self.set_heading(Some(c), &title);
                true
            }
            None => false,
        }
    }

    /// The body after the heading line, without leading blank lines.
    pub fn content(&self) -> &str {
        let rest = match &self.meta.heading {
            Some(h) => &self.body[h.end..],
            None => &self.body,
        };
        rest.trim_start_matches(['\n', '\r']).trim_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading() {
        let h = find_heading("\n# [X] tax return\nbody", Kind::Todo).unwrap();
        assert_eq!((h.checkbox, h.title.as_str()), (Some('x'), "tax return"));
        let h = find_heading("# [x] marks the spot\n", Kind::Note).unwrap();
        assert_eq!((h.checkbox, h.title.as_str()), (None, "[x] marks the spot"));
        assert!(find_heading("text\n# later\n", Kind::Note).is_none());
        assert!(find_heading("## sub\n", Kind::Note).is_none());
        assert_eq!(find_heading("# [ ]\n", Kind::Todo).unwrap().title, "");
    }

    #[test]
    fn status_and_checkbox() {
        let text = "---\nid: \"k3f9\"\nkind: todo\ncreated: 2026-10-16T21:58:03+11:00\ndate: 2026-10-16\ntags: [\"admin\"]\n---\n# [ ] call bank\nstuff\n";
        let mut it = Item::parse(Path::new("x.md"), Place::Root, text)
            .unwrap()
            .unwrap();
        assert_eq!(it.meta.current_status(), Some(Status::Open));
        it.push_status(parse_iso("2026-10-16").unwrap(), Status::Done);
        assert_eq!(it.meta.current_status(), Some(Status::Done));
        assert!(it.render().ends_with(
            "tags: [\"admin\"]\nstatus:\n  - date: 2026-10-16\n    status: done\n---\n# [x] call bank\nstuff\n"
        ));
    }
}

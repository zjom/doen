use std::fmt::Display;
use std::path::PathBuf;

use chrono::{Datelike, NaiveDate};

use crate::app::App;
use crate::dates::{Clock, format_iso, parse_date, weekday_short};
use crate::error::{Error, Result, general, usage};
use crate::frontmatter::{Doc, MANAGED};
use crate::item::{Kind, TODO_ONLY, parse_priority};
use crate::tags;

pub struct Template {
    pub name: String,
    pub doc: Doc,
    pub body: String,
}

const MANAGED_IN_TEMPLATE: [&str; 4] = ["scheduled", "due", "priority", "tags"];

/// The template directories, in order of precedence.
pub fn dirs(app: &App) -> [PathBuf; 2] {
    [
        app.root.join(".templates"),
        app.cfg.templates_dir(&app.root),
    ]
}

fn find(app: &App, name: &str) -> Option<PathBuf> {
    dirs(app)
        .into_iter()
        .map(|d| d.join(format!("{name}.md")))
        .find(|p| p.is_file())
}

/// Loads the template for `-T` (or the default template of the kind).
pub fn load(app: &App, kind: Kind, opt: Option<&str>) -> Result<Option<Template>> {
    let (name, path) = match opt {
        Some("none") => return Ok(None),
        Some(n) => (
            n,
            find(app, n).ok_or_else(|| usage(format!("template '{n}' not found")))?,
        ),
        None => match find(app, kind.as_str()) {
            Some(p) => (kind.as_str(), p),
            None => return Ok(None),
        },
    };
    let text = std::fs::read_to_string(&path)?;
    let (doc, body) = match Doc::parse_file(&text) {
        None => (Doc::empty(), text),
        Some(Ok(parsed)) => parsed,
        Some(Err(e)) => return Err(general(format!("{}: {e}", path.display()))),
    };
    if let Some(k) = doc
        .keys()
        .find(|k| MANAGED.contains(k) && !MANAGED_IN_TEMPLATE.contains(k))
    {
        return Err(general(format!(
            "{}: a template cannot set '{k}'",
            path.display()
        )));
    }
    if kind == Kind::Note
        && let Some(k) = TODO_ONLY.iter().find(|k| doc.get(k).is_some())
    {
        return Err(usage(format!(
            "template '{name}' sets '{k}', which a note cannot have"
        )));
    }
    Ok(Some(Template {
        name: name.to_string(),
        doc,
        body,
    }))
}

impl Template {
    fn error(&self, e: impl Display) -> Error {
        general(format!("template '{}': {e}", self.name))
    }

    pub fn tags(&self) -> Result<Vec<String>> {
        self.doc.get("tags").map_or(Ok(vec![]), |v| {
            tags::from_value(v).map_err(|e| self.error(e))
        })
    }

    /// A date field (`scheduled` or `due`). The value can be a relative date.
    pub fn date(&self, key: &str, clock: &Clock) -> Result<Option<NaiveDate>> {
        match self.doc.get_str(key) {
            None | Some("") => Ok(None),
            Some(s) => parse_date(s, clock).map(Some).map_err(|e| self.error(e)),
        }
    }

    pub fn priority(&self) -> Result<Option<u8>> {
        match self.doc.get_str("priority") {
            None | Some("") => Ok(None),
            Some(s) => parse_priority(s)
                .map(Some)
                .ok_or_else(|| self.error(format!("invalid priority '{s}'"))),
        }
    }

    /// The fields that the template copies into the new item.
    pub fn extra_keys(&self) -> Vec<String> {
        self.doc
            .keys()
            .filter(|k| !MANAGED_IN_TEMPLATE.contains(k))
            .map(String::from)
            .collect()
    }
}

/// Builds the body of a new item: a blank line, the heading (from the template or the title),
/// the template body and the content.
pub fn build_body(
    tmpl: Option<&str>,
    title: &str,
    date: NaiveDate,
    id: &str,
    kind: Kind,
    content: Option<&str>,
) -> String {
    let text = tmpl
        .unwrap_or("")
        .replace("{{title}}", title)
        .replace("{{date}}", &format_iso(date))
        .replace("{{weekday}}", &weekday_short(date.weekday()))
        .replace("{{id}}", id);
    let mut lines: Vec<&str> = text.lines().collect();
    let h1 = lines
        .iter()
        .position(|l| *l == "#" || l.starts_with("# ") || l.starts_with("#\t"));
    let heading_text = match h1 {
        Some(i) => {
            let t = lines.remove(i)[1..].trim().to_string();
            if lines[..i].iter().all(|l| l.trim().is_empty()) {
                lines.drain(..i);
            }
            t
        }
        None => title.to_string(),
    };
    let heading = match kind {
        Kind::Todo => format!("# [ ] {heading_text}"),
        Kind::Note => format!("# {heading_text}"),
    };
    let mut body = format!("\n{}\n", heading.trim_end());
    let rest = lines.join("\n");
    let rest = rest.trim_end();
    if !rest.is_empty() {
        body.push_str(rest);
        body.push('\n');
    }
    if let Some(c) = content {
        let c = c.trim_end_matches('\n');
        if !c.is_empty() {
            if !rest.is_empty() {
                body.push('\n');
            }
            body.push_str(c);
            body.push('\n');
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        assert_eq!(
            build_body(None, "buy milk", d, "k3f9", Kind::Todo, None),
            "\n# [ ] buy milk\n"
        );
        assert_eq!(
            build_body(None, "x", d, "k3f9", Kind::Note, Some("hello\n")),
            "\n# x\nhello\n"
        );
        let t = "# {{title}} ({{weekday}})\n\n## notes\n";
        assert_eq!(
            build_body(Some(t), "standup", d, "a", Kind::Note, None),
            "\n# standup (fri)\n\n## notes\n"
        );
        assert_eq!(
            build_body(Some("## a\n"), "t", d, "a", Kind::Todo, None),
            "\n# [ ] t\n## a\n"
        );
    }
}

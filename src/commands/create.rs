//! `note` and `todo`.

use std::io::Read;

use chrono::{SubsecRound, Timelike};

use crate::cli::args::CreateCmd;
use crate::commands::App;
use crate::commands::edit::{edit_again, edit_item, open_editor, require_terminal};
use crate::dates::parse_date;
use crate::error::{Result, general, usage, warn};
use crate::frontmatter::{Doc, Value};
use crate::ids;
use crate::item::{Item, Kind, Place};
use crate::tags;
use crate::template::{self, build_body};

/// Makes the filename slug from a title.
pub fn slug(title: &str) -> String {
    let ascii = deunicode::deunicode(title).to_lowercase();
    let mut s = String::new();
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_matches('-');
    if s.len() <= 40 {
        return s.to_string();
    }
    let cut = if s.as_bytes()[40] == b'-' {
        40
    } else {
        s[..40].rfind('-').unwrap_or(40)
    };
    s[..cut].trim_end_matches('-').to_string()
}

pub fn filename(item: &Item) -> String {
    let day = item.meta.created.format("%Y%m%d");
    match slug(item.title()) {
        s if s.is_empty() => format!("{day}-{}.md", item.id()),
        s => format!("{day}-{}-{s}.md", item.id()),
    }
}

pub fn run(app: &App, name: &str, c: CreateCmd) -> Result<()> {
    let kind = if name == "todo" {
        Kind::Todo
    } else {
        Kind::Note
    };
    let title = c.title.join(" ").trim().to_string();
    if kind == Kind::Note && c.due.is_some() {
        return Err(usage("-d applies only to todos"));
    }
    let tmpl = template::load(&app.root, kind, c.template.as_deref())?;
    let mut tag_list = app.cfg.default_tags.clone();
    if let Some(t) = &tmpl {
        tags::merge(&mut tag_list, t.tags()?);
    }
    for group in &c.tag {
        tags::merge(&mut tag_list, tags::parse_list(group).map_err(usage)?);
    }
    let date = match &c.date {
        Some(s) => parse_date(s, &app.clock).map_err(usage)?,
        None => app.clock.today,
    };
    let due = match &c.due {
        Some(s) => Some(parse_date(s, &app.clock).map_err(usage)?),
        None => match &tmpl {
            Some(t) => t.due(&app.clock)?,
            None => None,
        },
    };
    if due.is_some_and(|d| d < app.clock.today) {
        warn("the due date is in the past");
    }
    let content = match c.content.as_deref() {
        Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Some(s)
        }
        other => other.map(String::from),
    };
    if title.is_empty() {
        require_terminal()?;
    }

    let mut tx = app.begin()?;
    let id = ids::generate(&tx.store);
    let now = app
        .clock
        .now
        .trunc_subsecs(0)
        .with_nanosecond(0)
        .unwrap_or(app.clock.now);
    let mut doc = Doc::empty();
    doc.set("id", Value::Str(id.clone()), true);
    doc.set("kind", Value::Str(kind.as_str().into()), false);
    doc.set("created", Value::Str(now.to_rfc3339()), false);
    doc.set(
        "date",
        Value::Str(date.format("%Y-%m-%d").to_string()),
        false,
    );
    if let Some(d) = due {
        doc.set("due", Value::Str(d.format("%Y-%m-%d").to_string()), false);
    }
    doc.set(
        "tags",
        Value::Seq(tag_list.iter().map(|t| Value::Str(t.clone())).collect()),
        true,
    );
    if let Some(t) = &tmpl {
        for k in t.extra_keys() {
            doc.copy_from(&t.doc, &k);
        }
    }
    let body = build_body(
        tmpl.as_ref().map(|t| t.body.as_str()),
        &title,
        date,
        &id,
        kind,
        content.as_deref(),
    );
    let text = format!("{}{body}", doc.render());
    let placeholder = app.root.join("new.md");
    let mut item = Item::parse(&placeholder, Place::Root, &text)
        .map_err(general)?
        .ok_or_else(|| general("internal error: new item has no id"))?;

    if title.is_empty() {
        // Release the lock while the editor is open.
        drop(tx);
        item = edit_new(app, item)?;
        tx = app.begin()?;
        if tx.store.has_id(item.id()) {
            let new_id = ids::generate(&tx.store);
            item.set_id(&new_id);
        }
    }
    item.path = app.root.join(filename(&item));
    tx.store.write(&mut item, &mut tx.changes)?;
    tx.changes.quiet_lines.push(format!(
        "{}  {}  {}",
        kind.as_str(),
        item.id(),
        item.title()
    ));
    println!("{}", item.id());
    let id = item.id().to_string();
    tx.store.items.push(item);
    tx.finish(app)?;
    if c.edit {
        edit_item(app, &id, true)?;
    }
    Ok(())
}

/// Opens the editor on a new item that has no title yet.
fn edit_new(app: &App, item: Item) -> Result<Item> {
    let tmp = tempfile::Builder::new()
        .prefix("doen-new-")
        .suffix(".md")
        .tempfile()?;
    std::fs::write(tmp.path(), item.render())?;
    loop {
        open_editor(app, tmp.path())?;
        let text = std::fs::read_to_string(tmp.path())?;
        match crate::commands::edit::validate(&item, &text) {
            Ok(new) if new.meta.heading.as_ref().is_none_or(|h| h.title.is_empty()) => {
                return Err(general("the title is empty: no item created"));
            }
            Ok(mut new) => {
                new.sync_checkbox();
                return Ok(new);
            }
            Err(e) => edit_again(&format!("invalid frontmatter: {e}"))?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs() {
        assert_eq!(slug("Call bank!"), "call-bank");
        assert_eq!(slug("Café über straße"), "cafe-uber-strasse");
        assert_eq!(slug("!!!"), "");
        assert_eq!(
            slug("a very long title that goes on and on beyond forty"),
            "a-very-long-title-that-goes-on-and-on"
        );
        assert_eq!(slug(&"x".repeat(50)), "x".repeat(40));
    }
}

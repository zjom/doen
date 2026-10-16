//! `note` and `todo`.

use std::io::Read;
use std::path::PathBuf;

use chrono::SubsecRound;

use crate::app::App;
use crate::cli::args::CreateCmd;
use crate::commands::edit::edit_item;
use crate::dates::{format_iso, parse_date};
use crate::editor;
use crate::error::{Result, general, usage, warn};
use crate::frontmatter::{Doc, Value};
use crate::ids;
use crate::item::{Item, Kind, Place};
use crate::store::{Change, filename};
use crate::tags;
use crate::template::{self, build_body};

pub fn run(app: &App, kind: Kind, c: CreateCmd) -> Result<()> {
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
        editor::require_terminal()?;
    }

    let mut tx = app.begin()?;
    let id = ids::generate(&tx.store);
    let now = app.clock.now.trunc_subsecs(0);
    let mut doc = Doc::empty();
    doc.set("id", Value::Str(id.clone()), true);
    doc.set("kind", Value::Str(kind.as_str().into()), false);
    doc.set("created", Value::Str(now.to_rfc3339()), false);
    doc.set("date", Value::Str(format_iso(date)), false);
    if let Some(d) = due {
        doc.set("due", Value::Str(format_iso(d)), false);
    }
    doc.set("tags", tags::to_value(&tag_list), true);
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
    let mut item = Item::new(PathBuf::new(), Place::Root, doc, body).map_err(general)?;

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
    let id = item.id().to_string();
    tx.changes.quiet.push(Change::new(kind.as_str(), &item));
    tx.store.items.push(item);
    tx.store.write(tx.store.items.len() - 1, &mut tx.changes)?;
    println!("{id}");
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
        editor::open(app, tmp.path())?;
        let text = std::fs::read_to_string(tmp.path())?;
        match editor::validate(&item, &text) {
            Ok(new) if !new.has_title() => {
                return Err(general("the title is empty: no item created"));
            }
            Ok(mut new) => {
                new.sync_checkbox();
                return Ok(new);
            }
            Err(e) => editor::edit_again(&format!("invalid frontmatter: {e}"))?,
        }
    }
}

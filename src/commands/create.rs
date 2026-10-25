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
    let edit = c.edit;
    let id = create(app, kind, c)?;
    println!("{id}");
    if edit {
        edit_item(app, &id, true)?;
    }
    Ok(())
}

/// Creates an item and returns its ID. Prints nothing; `c.edit` is ignored.
pub fn create(app: &App, kind: Kind, c: CreateCmd) -> Result<String> {
    let title = c.title.join(" ").trim().to_string();
    let todo_only = [
        (c.scheduled.is_some(), "-S"),
        (c.due.is_some(), "-d"),
        (c.priority.is_some(), "-p"),
    ];
    if kind == Kind::Note
        && let Some((_, flag)) = todo_only.iter().find(|(set, _)| *set)
    {
        return Err(usage(format!("{flag} applies only to todos")));
    }
    let tmpl = template::load(app, kind, c.template.as_deref())?;
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
    let date_opt = |opt: &Option<String>, key: &str| match opt {
        Some(s) => parse_date(s, &app.clock).map(Some).map_err(usage),
        None => match &tmpl {
            Some(t) => t.date(key, &app.clock),
            None => Ok(None),
        },
    };
    let scheduled = date_opt(&c.scheduled, "scheduled")?;
    let due = date_opt(&c.due, "due")?;
    let priority = match (c.priority, &tmpl) {
        (Some(p), _) => Some(p),
        (None, Some(t)) => t.priority()?,
        (None, None) => None,
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
    for (key, d) in [("date", Some(date)), ("scheduled", scheduled), ("due", due)] {
        if let Some(d) = d {
            doc.set(key, Value::Str(format_iso(d)), false);
        }
    }
    if let Some(p) = priority {
        doc.set("priority", Value::Str(p.to_string()), false);
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
    tx.finish(app)?;
    Ok(id)
}

fn edit_new(app: &App, item: Item) -> Result<Item> {
    let tmp = editor::temp_file(&app.root, "new", &item.render())?;
    let (_, mut new) = editor::edit_valid(app, tmp.path(), &item)?;
    if !new.has_title() {
        return Err(general("the title is empty: no item created"));
    }
    new.sync_checkbox();
    Ok(new)
}

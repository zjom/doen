use std::io::Read;
use std::path::PathBuf;

use chrono::{NaiveDate, SubsecRound};

use crate::app::{App, Tx, print_changes};
use crate::cli::args::CreateCmd;
use crate::commands::edit::edit_item;
use crate::dates::{format_iso, parse_date};
use crate::editor;
use crate::error::{Result, general, usage};
use crate::frontmatter::{Doc, Value};
use crate::ids;
use crate::item::{Item, Kind, Place};
use crate::store::{Change, filename};
use crate::tags;
use crate::template::{self, Template, build_body};

pub fn run(app: &App, kind: Kind, c: CreateCmd) -> Result<()> {
    let edit = c.edit;
    let draft = Draft::new(app, kind, c)?;
    let id = if draft.title.is_empty() {
        editor::require_terminal()?;
        // The ID is checked again after the edit, under the lock.
        let id = ids::generate(&app.store(&[])?);
        let item = edit_new(app, draft.build(app, &id)?)?;
        app.run_tx(&[], |tx| insert(app, tx, item))?
    } else {
        app.run_tx(&[], |tx| create(app, tx, &draft))?
    };
    println!("{id}");
    if edit {
        let mut changes = edit_item(app, &id)?;
        changes.shown.clear();
        print_changes(&mut changes);
    }
    Ok(())
}

/// A new item as the options describe it, checked before the transaction.
pub struct Draft {
    kind: Kind,
    title: String,
    tmpl: Option<Template>,
    tags: Vec<String>,
    date: NaiveDate,
    scheduled: Option<NaiveDate>,
    due: Option<NaiveDate>,
    priority: Option<u8>,
    content: Option<String>,
}

impl Draft {
    /// Checks the options and reads the template, and stdin for `-c -`. `c.edit` is ignored.
    pub fn new(app: &App, kind: Kind, c: CreateCmd) -> Result<Draft> {
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
        let content = match c.content.as_deref() {
            Some("-") => {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                Some(s)
            }
            other => other.map(String::from),
        };
        Ok(Draft {
            kind,
            title,
            tmpl,
            tags: tag_list,
            date,
            scheduled,
            due,
            priority,
            content,
        })
    }

    /// The item with the ID `id`, without a path.
    fn build(&self, app: &App, id: &str) -> Result<Item> {
        let now = app.clock.now.trunc_subsecs(0);
        let mut doc = Doc::empty();
        doc.set("id", Value::Str(id.into()), true);
        doc.set("kind", Value::Str(self.kind.as_str().into()), false);
        doc.set("created", Value::Str(now.to_rfc3339()), false);
        let dates = [
            ("date", Some(self.date)),
            ("scheduled", self.scheduled),
            ("due", self.due),
        ];
        for (key, d) in dates {
            if let Some(d) = d {
                doc.set(key, Value::Str(format_iso(d)), false);
            }
        }
        if let Some(p) = self.priority {
            doc.set("priority", Value::Str(p.to_string()), false);
        }
        doc.set("tags", tags::to_value(&self.tags), true);
        if let Some(t) = &self.tmpl {
            for k in t.extra_keys() {
                doc.copy_from(&t.doc, &k);
            }
        }
        let body = build_body(
            self.tmpl.as_ref().map(|t| t.body.as_str()),
            &self.title,
            self.date,
            id,
            self.kind,
            self.content.as_deref(),
        );
        Item::new(PathBuf::new(), Place::Root, doc, body).map_err(general)
    }
}

/// Creates the item of a draft with a title, and returns its ID.
pub fn create(app: &App, tx: &mut Tx, draft: &Draft) -> Result<String> {
    if draft.title.is_empty() {
        return Err(usage("the title cannot be empty"));
    }
    let item = draft.build(app, &ids::generate(&tx.store))?;
    insert(app, tx, item)
}

/// Writes a new item, with a new ID if its ID is taken.
fn insert(app: &App, tx: &mut Tx, mut item: Item) -> Result<String> {
    if tx.store.has_id(item.id()) {
        item.set_id(&ids::generate(&tx.store));
    }
    if item.meta.due.is_some_and(|d| d < app.clock.today) {
        tx.changes.warn("the due date is in the past");
    }
    item.path = app.root.join(filename(&item));
    let id = item.id().to_string();
    tx.changes
        .quiet
        .push(Change::new(item.kind().as_str(), &item));
    tx.store.items.push(item);
    tx.store.write(tx.store.items.len() - 1, &mut tx.changes)?;
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

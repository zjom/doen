use chrono::NaiveDate;

use crate::app::App;
use crate::cli::args::{DateCmd, PriorityCmd, TagArgs, TitleCmd};
use crate::commands::{require_todos, resolve_ids};
use crate::dates::{Clock, parse_date};
use crate::error::{Result, usage, warn};
use crate::item::{Item, Kind, Meta, parse_priority_or_none};
use crate::tags;

pub fn title(app: &App, c: TitleCmd) -> Result<()> {
    let new_title = c.title.join(" ").trim().to_string();
    if new_title.is_empty() {
        return Err(usage("the title cannot be empty"));
    }
    let mut tx = app.begin()?;
    let n = resolve_ids(&mut tx.store, std::slice::from_ref(&c.id), false)?[0];
    let item = &mut tx.store.items[n];
    if item.meta.heading.is_some() && item.title() == new_title {
        warn(format!("{}: the title is already '{new_title}'", item.id()));
        return tx.finish(app);
    }
    retitle(item, &new_title);
    tx.save(n, "title")?;
    tx.finish(app)
}

/// Rewrites the H1 heading and keeps the checkbox.
pub fn retitle(item: &mut Item, title: &str) {
    let checkbox = match item.kind() {
        Kind::Note => None,
        Kind::Todo => item
            .meta
            .heading
            .as_ref()
            .and_then(|h| h.checkbox)
            .or_else(|| item.meta.current_status().map(|s| s.checkbox())),
    };
    item.set_heading(checkbox, title);
}

#[derive(Clone, Copy, PartialEq)]
pub enum DateField {
    Scheduled,
    Due,
    Date,
}

impl DateField {
    pub fn key(self) -> &'static str {
        match self {
            DateField::Scheduled => "scheduled",
            DateField::Due => "due",
            DateField::Date => "date",
        }
    }

    /// Only `date` is required; the todo-only fields can be cleared.
    pub fn todo_only(self) -> bool {
        self != DateField::Date
    }

    pub fn get(self, m: &Meta) -> Option<NaiveDate> {
        match self {
            DateField::Scheduled => m.scheduled,
            DateField::Due => m.due,
            DateField::Date => Some(m.date),
        }
    }

    /// `none` clears a todo-only field.
    pub fn parse(self, s: &str, clock: &Clock) -> Result<Option<NaiveDate>> {
        if self.todo_only() && s == "none" {
            return Ok(None);
        }
        parse_date(s, clock).map(Some).map_err(usage)
    }
}

pub fn date(app: &App, field: DateField, c: DateCmd) -> Result<()> {
    let value = field.parse(&c.date, &app.clock)?;
    let mut tx = app.begin()?;
    let idx = resolve_ids(&mut tx.store, &c.id, false)?;
    if field.todo_only() {
        require_todos(&tx.store, &idx, &format!("a {} date", field.key()))?;
    }
    if field == DateField::Due && value.is_some_and(|d| d < app.clock.today) {
        warn("the due date is in the past");
    }
    for n in idx {
        let item = &mut tx.store.items[n];
        if field.get(&item.meta) == value {
            warn(format!(
                "{}: {} is already {}",
                item.id(),
                field.key(),
                c.date
            ));
            continue;
        }
        item.set_date(field.key(), value);
        tx.save(n, field.key())?;
    }
    tx.finish(app)
}

pub fn priority(app: &App, c: PriorityCmd) -> Result<()> {
    let value = parse_priority_or_none(&c.priority).map_err(usage)?;
    let mut tx = app.begin()?;
    let idx = resolve_ids(&mut tx.store, &c.id, false)?;
    require_todos(&tx.store, &idx, "a priority")?;
    for n in idx {
        let item = &mut tx.store.items[n];
        if item.meta.priority == value {
            warn(format!("{}: priority is already {}", item.id(), c.priority));
            continue;
        }
        item.set_priority(value);
        tx.save(n, "priority")?;
    }
    tx.finish(app)
}

pub fn tag(app: &App, add: bool, c: TagArgs) -> Result<()> {
    let list = tags::parse_list(&c.tags).map_err(usage)?;
    if list.is_empty() {
        return Err(usage("no tags given"));
    }
    let mut tx = app.begin()?;
    let idx = resolve_ids(&mut tx.store, &c.id, false)?;
    for n in idx {
        let item = &mut tx.store.items[n];
        let mut new = item.meta.tags.clone();
        if add {
            tags::merge(&mut new, list.iter().cloned());
        } else {
            new.retain(|t| !list.contains(t));
        }
        if new == item.meta.tags {
            let what = if add { "already has" } else { "does not have" };
            warn(format!("{}: the item {what} these tags", item.id()));
            continue;
        }
        item.set_tags(&new);
        tx.save(n, if add { "tag" } else { "untag" })?;
    }
    tx.finish(app)
}

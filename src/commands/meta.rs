use chrono::NaiveDate;

use crate::app::Tx;
use crate::commands::{only_todos, require_todos};
use crate::error::{Result, usage};
pub use crate::item::DateField;
use crate::item::{Item, Kind};
use crate::tags;

/// The title of the title words, which cannot be empty.
pub fn parse_title(words: &[String]) -> Result<String> {
    let title = words.join(" ").trim().to_string();
    if title.is_empty() {
        return Err(usage("the title cannot be empty"));
    }
    Ok(title)
}

pub fn set_title(tx: &mut Tx, n: usize, title: &str) -> Result<()> {
    let item = &mut tx.store.items[n];
    if item.meta.heading.is_some() && item.title() == title {
        let msg = format!("{}: the title is already '{title}'", item.id());
        tx.changes.warn(msg);
        return Ok(());
    }
    retitle(item, title);
    tx.save(n, "title")
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

/// Sets a date of the items. `text` is the date as given, for the warnings.
pub fn set_date(
    tx: &mut Tx,
    field: DateField,
    value: Option<NaiveDate>,
    text: &str,
    idx: &[usize],
    today: NaiveDate,
) -> Result<()> {
    let what = format!("a {} date", field.key());
    if field.todo_only() {
        require_todos(&tx.store, idx, &what)?;
    }
    if field == DateField::Due && value.is_some_and(|d| d < today) {
        tx.changes.warn("the due date is in the past");
    }
    for &n in idx {
        let item = &mut tx.store.items[n];
        if field.get(&item.meta) == value {
            let msg = format!("{}: {} is already {text}", item.id(), field.key());
            tx.changes.warn(msg);
            continue;
        }
        item.set_date(field, value)
            .map_err(|_| only_todos(item.id(), &what))?;
        tx.save(n, field.key())?;
    }
    Ok(())
}

pub fn set_priority(tx: &mut Tx, value: Option<u8>, idx: &[usize]) -> Result<()> {
    require_todos(&tx.store, idx, "a priority")?;
    for &n in idx {
        let item = &mut tx.store.items[n];
        if item.meta.priority == value {
            let p = value.map_or("none".to_string(), |p| p.to_string());
            tx.changes
                .warn(format!("{}: priority is already {p}", item.id()));
            continue;
        }
        item.set_priority(value)
            .map_err(|_| only_todos(item.id(), "a priority"))?;
        tx.save(n, "priority")?;
    }
    Ok(())
}

/// The tags of a comma-separated list, which cannot be empty.
pub fn parse_tags(s: &str) -> Result<Vec<String>> {
    let list = tags::parse_list(s).map_err(usage)?;
    if list.is_empty() {
        return Err(usage("no tags given"));
    }
    Ok(list)
}

/// Adds the tags to the items, or removes them.
pub fn tag(tx: &mut Tx, add: bool, list: &[String], idx: &[usize]) -> Result<()> {
    for &n in idx {
        let item = &mut tx.store.items[n];
        let mut new = item.meta.tags.clone();
        if add {
            tags::merge(&mut new, list.iter().cloned());
        } else {
            new.retain(|t| !list.contains(t));
        }
        if new == item.meta.tags {
            let what = if add { "already has" } else { "does not have" };
            let msg = format!("{}: the item {what} these tags", item.id());
            tx.changes.warn(msg);
            continue;
        }
        item.set_tags(&new);
        tx.save(n, if add { "tag" } else { "untag" })?;
    }
    Ok(())
}

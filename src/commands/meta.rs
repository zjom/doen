use crate::app::App;
use crate::cli::args::{DateCmd, TagArgs, TitleCmd};
use crate::commands::resolve_ids;
use crate::dates::parse_date;
use crate::error::{Result, usage, warn};
use crate::item::Kind;
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
    let checkbox = match item.kind() {
        Kind::Note => None,
        Kind::Todo => item
            .meta
            .heading
            .as_ref()
            .and_then(|h| h.checkbox)
            .or_else(|| item.meta.current_status().map(|s| s.checkbox())),
    };
    item.set_heading(checkbox, &new_title);
    tx.save(n, "title")?;
    tx.finish(app)
}

#[derive(Clone, Copy, PartialEq)]
pub enum DateField {
    Due,
    Date,
}

impl DateField {
    fn key(self) -> &'static str {
        match self {
            DateField::Due => "due",
            DateField::Date => "date",
        }
    }
}

pub fn date(app: &App, field: DateField, c: DateCmd) -> Result<()> {
    let value = if field == DateField::Due && c.date == "none" {
        None
    } else {
        Some(parse_date(&c.date, &app.clock).map_err(usage)?)
    };
    let mut tx = app.begin()?;
    let idx = resolve_ids(&mut tx.store, &c.id, false)?;
    if field == DateField::Due
        && let Some(&n) = idx
            .iter()
            .find(|&&n| tx.store.items[n].kind() == Kind::Note)
    {
        return Err(usage(format!(
            "{} is a note: only todos have a due date",
            tx.store.items[n].id()
        )));
    }
    if field == DateField::Due && value.is_some_and(|d| d < app.clock.today) {
        warn("the due date is in the past");
    }
    for n in idx {
        let item = &mut tx.store.items[n];
        let current = match field {
            DateField::Due => item.meta.due,
            DateField::Date => Some(item.meta.date),
        };
        if current == value {
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

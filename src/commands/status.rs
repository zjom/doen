use chrono::NaiveDate;

use crate::app::App;
use crate::cli::args::StatusCmd;
use crate::commands::{only_todos, require_todos, resolve_ids};
use crate::dates::parse_date;
use crate::error::{Result, usage, warn};
use crate::item::{Item, NotATodo, Status};

pub fn run(app: &App, status: Status, c: StatusCmd) -> Result<()> {
    let on = match &c.on {
        Some(s) => parse_date(s, &app.clock).map_err(usage)?,
        None => app.clock.today,
    };
    if on > app.clock.today {
        return Err(usage("the date of a status change cannot be in the future"));
    }
    let mut tx = app.begin()?;
    let idx = resolve_ids(&mut tx.store, &c.id, false)?;
    require_todos(&tx.store, &idx, "a status")?;
    for &n in &idx {
        let item = &tx.store.items[n];
        if item.meta.current_status() != Some(status)
            && let Some((last, _)) = item.meta.status.last()
            && on < *last
        {
            return Err(usage(format!(
                "{}: the date {on} is before the previous status change ({last})",
                item.id()
            )));
        }
    }
    for n in idx {
        let item = &tx.store.items[n];
        if item.meta.current_status() == Some(status) {
            warn(format!("{} is already {}", item.id(), status.as_str()));
            continue;
        }
        apply(app, &mut tx.store.items[n], status, on)
            .map_err(|_| only_todos(&tx.store.items[n].meta.id, "a status"))?;
        tx.save(n, status.verb())?;
    }
    tx.finish(app)
}

/// Adds a status entry and archives or unarchives the todo.
pub fn apply(
    app: &App,
    item: &mut Item,
    status: Status,
    on: NaiveDate,
) -> std::result::Result<(), NotATodo> {
    item.push_status(on, status)?;
    match status {
        Status::Open => item.unarchive(),
        _ if app.cfg.archive_closed && item.meta.archived.is_none() => item.archive(on),
        _ => {}
    }
    Ok(())
}

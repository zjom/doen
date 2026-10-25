use chrono::NaiveDate;

use crate::app::Tx;
use crate::commands::{only_todos, require_todos};
use crate::config::Config;
use crate::dates::{Clock, parse_date};
use crate::error::{Result, usage};
use crate::item::{Item, NotATodo, Status};

/// The date of a status change: `--on`, or today. It cannot be in the future.
pub fn parse_on(on: Option<&str>, clock: &Clock) -> Result<NaiveDate> {
    let on = match on {
        Some(s) => parse_date(s, clock).map_err(usage)?,
        None => clock.today,
    };
    if on > clock.today {
        return Err(usage("the date of a status change cannot be in the future"));
    }
    Ok(on)
}

pub fn set(tx: &mut Tx, cfg: &Config, status: Status, on: NaiveDate, idx: &[usize]) -> Result<()> {
    require_todos(&tx.store, idx, "a status")?;
    for &n in idx {
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
    for &n in idx {
        let item = &mut tx.store.items[n];
        if item.meta.current_status() == Some(status) {
            tx.changes
                .warn(format!("{} is already {}", item.id(), status.as_str()));
            continue;
        }
        apply(cfg, item, status, on).map_err(|_| only_todos(item.id(), "a status"))?;
        tx.save(n, status.verb())?;
    }
    Ok(())
}

/// Adds a status entry and archives or unarchives the todo.
pub fn apply(
    cfg: &Config,
    item: &mut Item,
    status: Status,
    on: NaiveDate,
) -> std::result::Result<(), NotATodo> {
    item.push_status(on, status)?;
    match status {
        Status::Open => item.unarchive(),
        _ if cfg.archive_closed && item.meta.archived.is_none() => item.archive(on),
        _ => {}
    }
    Ok(())
}

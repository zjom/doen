//! `done`, `drop` and `reopen`.

use chrono::NaiveDate;

use crate::cli::args::StatusCmd;
use crate::commands::{App, Tx, resolve};
use crate::dates::parse_date;
use crate::error::{Result, usage, warn};
use crate::item::{Kind, Status};

pub fn run(app: &App, name: &str, c: StatusCmd) -> Result<()> {
    let status =
        Status::parse(if name == "reopen" { "open" } else { name }).expect("status command");
    let on = match &c.on {
        Some(s) => parse_date(s, &app.clock).map_err(usage)?,
        None => app.clock.today,
    };
    if on > app.clock.today {
        return Err(usage("the date of a status change cannot be in the future"));
    }
    let mut tx = app.begin()?;
    let idx = resolve(&tx, &c.id, false)?;
    for &n in &idx {
        let item = &tx.store.items[n];
        if item.kind() == Kind::Note {
            return Err(usage(format!(
                "{} is a note: only todos have a status",
                item.id()
            )));
        }
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
        apply(app, &mut tx, n, status, on);
        tx.save(n, name)?;
    }
    tx.finish(app)
}

/// Adds a status entry and archives or unarchives the todo. Shared with `doen edit`.
pub fn apply(app: &App, tx: &mut Tx, n: usize, status: Status, on: NaiveDate) {
    let item = &mut tx.store.items[n];
    item.push_status(on, status);
    apply_archive(app, item, status, on);
}

pub fn apply_archive(app: &App, item: &mut crate::item::Item, status: Status, on: NaiveDate) {
    match status {
        Status::Open => item.set_date("archived", None),
        _ if app.cfg.archive_closed && item.meta.archived.is_none() => {
            item.set_date("archived", Some(on))
        }
        _ => {}
    }
}

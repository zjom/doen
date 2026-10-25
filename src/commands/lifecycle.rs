use crate::app::{App, Tx};
use crate::cli::args::{IdsCmd, RmCmd};
use crate::commands::resolve_ids;
use crate::error::Result;
use crate::item::{Item, Place};

#[derive(Clone, Copy, PartialEq)]
pub enum Lifecycle {
    Archive,
    Unarchive,
    Trash,
    Restore,
}

impl Lifecycle {
    fn verb(self) -> &'static str {
        match self {
            Lifecycle::Archive => "archive",
            Lifecycle::Unarchive => "unarchive",
            Lifecycle::Trash => "rm",
            Lifecycle::Restore => "restore",
        }
    }

    /// Why the change does nothing to `item`, if it does nothing.
    fn noop(self, item: &Item) -> Option<&'static str> {
        match self {
            Lifecycle::Archive => item.meta.archived.is_some().then_some("already archived"),
            Lifecycle::Unarchive => item.meta.archived.is_none().then_some("not archived"),
            Lifecycle::Trash => None,
            Lifecycle::Restore => item.meta.trashed.is_none().then_some("not in the trash"),
        }
    }
}

fn confirm(app: &App, tx: &mut Tx, idx: &[usize], verb: &str, rest: &str) -> Result<()> {
    let these = match idx.len() {
        1 => "this item".to_string(),
        n => format!("these {n} items"),
    };
    app.confirm(tx, idx, &format!("{verb} {these}{rest}?"))
}

pub fn run(app: &App, op: Lifecycle, c: IdsCmd) -> Result<()> {
    app.run_tx(&[], |tx| {
        let idx = resolve_ids(&mut tx.store, &c.id, op == Lifecycle::Restore)?;
        if op == Lifecycle::Trash {
            confirm(app, tx, &idx, "Move", " to the trash")?;
        }
        let today = app.clock.today;
        for n in idx {
            let item = &mut tx.store.items[n];
            if let Some(why) = op.noop(item) {
                tx.changes.warn(format!("{} is {why}", item.id()));
                continue;
            }
            match op {
                Lifecycle::Archive => item.archive(today),
                Lifecycle::Unarchive => item.unarchive(),
                Lifecycle::Trash => item.trash(today),
                Lifecycle::Restore => item.restore(),
            }
            tx.save(n, op.verb())?;
        }
        Ok(())
    })
}

/// `rm --force`: deletes the items permanently, including items already in the trash.
pub fn delete(app: &App, c: RmCmd) -> Result<()> {
    app.run_tx(&[], |tx| {
        let idx = resolve_ids(&mut tx.store, &c.id, true)?;
        confirm(app, tx, &idx, "Delete", " permanently")?;
        tx.delete(&idx)
    })
}

pub fn empty_trash(app: &App) -> Result<()> {
    app.run_tx(&[], |tx| {
        let idx: Vec<usize> = (0..tx.store.items.len())
            .filter(|&n| tx.store.items[n].place == Place::Trash)
            .collect();
        if idx.is_empty() {
            tx.changes.notice("the trash is empty");
            return Ok(());
        }
        confirm(app, tx, &idx, "Delete", " permanently")?;
        tx.delete(&idx)
    })
}

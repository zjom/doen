use chrono::NaiveDate;

use crate::app::Tx;
use crate::error::Result;
use crate::item::{Item, Place};
use crate::store::Store;

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

/// The confirmation question for `n` items, as `Move this item to the trash?`.
pub fn question(verb: &str, n: usize, rest: &str) -> String {
    let these = match n {
        1 => "this item".to_string(),
        n => format!("these {n} items"),
    };
    format!("{verb} {these}{rest}?")
}

pub fn apply(tx: &mut Tx, op: Lifecycle, idx: &[usize], today: NaiveDate) -> Result<()> {
    for &n in idx {
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
}

/// Deletes the items permanently, including items already in the trash.
pub fn delete(tx: &mut Tx, idx: &[usize]) -> Result<()> {
    tx.delete(idx)
}

/// The items in the trash.
pub fn trashed(store: &Store) -> Vec<usize> {
    (0..store.items.len())
        .filter(|&n| store.items[n].place == Place::Trash)
        .collect()
}

pub fn empty_trash(tx: &mut Tx) -> Result<()> {
    let idx = trashed(&tx.store);
    if idx.is_empty() {
        tx.changes.notice("the trash is empty");
        return Ok(());
    }
    tx.delete(&idx)
}

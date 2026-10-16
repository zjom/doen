use crate::cli::args::IdsCmd;
use crate::commands::{App, item_line, resolve};
use crate::error::{Result, warn};
use crate::item::Item;

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

    fn field(self) -> &'static str {
        match self {
            Lifecycle::Archive | Lifecycle::Unarchive => "archived",
            Lifecycle::Trash | Lifecycle::Restore => "trashed",
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

pub fn run(app: &App, op: Lifecycle, c: IdsCmd) -> Result<()> {
    let mut tx = app.begin()?;
    let idx = resolve(&tx, &c.id, op == Lifecycle::Restore)?;
    if op == Lifecycle::Trash {
        for &n in &idx {
            eprintln!("  {}", item_line(&tx.store.items[n]));
        }
        let what = if idx.len() == 1 {
            "this item".to_string()
        } else {
            format!("these {} items", idx.len())
        };
        app.confirm(&format!("Move {what} to the trash?"))?;
    }
    let set = matches!(op, Lifecycle::Archive | Lifecycle::Trash).then_some(app.clock.today);
    for n in idx {
        let item = &mut tx.store.items[n];
        if let Some(why) = op.noop(item) {
            warn(format!("{} is {why}", item.id()));
            continue;
        }
        item.set_date(op.field(), set);
        tx.save(n, op.verb())?;
    }
    tx.finish(app)
}

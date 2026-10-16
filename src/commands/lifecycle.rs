//! `archive`, `unarchive`, `rm` and `restore`.

use crate::cli::args::IdsCmd;
use crate::commands::{App, item_line, resolve};
use crate::error::{Result, warn};

pub fn run(app: &App, name: &str, c: IdsCmd) -> Result<()> {
    let today = app.clock.today;
    let mut tx = app.begin()?;
    let idx = resolve(&tx, &c.id, name == "restore")?;
    if name == "rm" {
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
    for n in idx {
        let item = &mut tx.store.items[n];
        let (field, value, skip) = match name {
            "archive" => (
                "archived",
                Some(today),
                item.meta.archived.is_some().then_some("already archived"),
            ),
            "unarchive" => (
                "archived",
                None,
                item.meta.archived.is_none().then_some("not archived"),
            ),
            "rm" => ("trashed", Some(today), None),
            "restore" => (
                "trashed",
                None,
                item.meta.trashed.is_none().then_some("not in the trash"),
            ),
            _ => unreachable!(),
        };
        if let Some(why) = skip {
            warn(format!("{} is {why}", item.id()));
            continue;
        }
        item.set_date(field, value);
        tx.save(n, name)?;
    }
    tx.finish(app)
}

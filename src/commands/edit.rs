use crate::app::App;
use crate::commands::status;
use crate::editor::{self, edit_again};
use crate::error::{Result, general, warn};
use crate::ids;
use crate::item::{Item, Kind, Status};

/// The lock is held only while doen saves the file.
pub fn edit_item(app: &App, id: &str, quiet: bool) -> Result<()> {
    editor::require_terminal()?;
    let mut store = app.store(&[])?;
    let n = ids::resolve(&mut store, id, false)?;
    let mut orig_item = store.items.swap_remove(n);
    let id = orig_item.id().to_string();
    let mut orig_text = std::fs::read_to_string(&orig_item.path)?;
    let dir = orig_item.path.parent().unwrap_or(app.root.as_path());
    let tmp = editor::temp_file(dir, &id, &orig_text)?;
    loop {
        let (text, edited) = editor::edit_valid(app, tmp.path(), &orig_item)?;
        if text == orig_text {
            eprintln!("doen: no changes");
            return Ok(());
        }
        let mut tx = app.begin()?;
        let n = ids::resolve(&mut tx.store, &id, false)?;
        let current = std::fs::read_to_string(&tx.store.items[n].path)?;
        if current != orig_text {
            drop(tx);
            edit_again("the file changed while the editor was open")?;
            // The next save compares with the new version of the file.
            orig_text = current;
            let mut store = app.store(&[])?;
            let n =
                ids::resolve(&mut store, &id, false).map_err(|_| general("the item is gone"))?;
            orig_item = store.items.swap_remove(n);
            continue;
        }
        let mut item = edited;
        item.path = tx.store.items[n].path.clone();
        item.place = tx.store.items[n].place;
        reconcile(app, &orig_item, &mut item);
        tx.store.items[n] = item;
        tx.save(n, "edit")?;
        if quiet {
            tx.changes.hush();
        }
        return tx.finish(app);
    }
}

/// Reconciles the heading checkbox with the status log after an edit.
pub fn reconcile(app: &App, before: &Item, after: &mut Item) {
    if after.kind() != Kind::Todo {
        return;
    }
    let log_changed = before.meta.status != after.meta.status;
    let cb_before = before.meta.heading.as_ref().and_then(|h| h.checkbox);
    let cb_after = after.meta.heading.as_ref().and_then(|h| h.checkbox);
    // Only the checkbox changed: apply the matching status command.
    if !log_changed
        && cb_before != cb_after
        && let Some(c) = cb_after
        && after.meta.current_status() != Some(Status::from_checkbox(c))
    {
        status::apply(app, after, Status::from_checkbox(c), app.clock.today);
        return;
    }
    if log_changed && after.checkbox_mismatch().is_some() {
        warn(format!(
            "{}: the checkbox disagreed with the status log; doen rewrote the checkbox",
            after.id()
        ));
    }
    after.sync_checkbox();
}

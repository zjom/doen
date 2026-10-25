use crate::app::{App, print_changes};
use crate::commands::status;
use crate::editor::{self, edit_again};
use crate::error::{Result, general};
use crate::ids;
use crate::item::{Item, Kind, Status};
use crate::store::Changes;

/// `doen edit`: prints the output line and the notices.
pub fn run(app: &App, id: &str) -> Result<()> {
    print_changes(&mut edit_item(app, id)?);
    Ok(())
}

/// Opens the item in the editor and saves the change. The lock is held only while doen saves
/// the file.
pub fn edit_item(app: &App, id: &str) -> Result<Changes> {
    editor::require_terminal()?;
    let mut store = app.store(&[])?;
    let n = ids::resolve(&mut store, id, false)?;
    let mut orig_item = store.items.swap_remove(n);
    let id = orig_item.id().to_string();
    let mut orig_text = std::fs::read_to_string(&orig_item.path)?;
    let dir = orig_item.path.parent().unwrap_or(app.root.as_path());
    let tmp = editor::temp_file(dir, &id, &orig_text)?;
    // The notices of the transactions that did not save.
    let mut notices = vec![];
    loop {
        let (text, edited) = editor::edit_valid(app, tmp.path(), &orig_item)?;
        if text == orig_text {
            notices.push("no changes".to_string());
            return Ok(Changes {
                notices,
                ..Changes::default()
            });
        }
        let mut tx = app.begin()?;
        let n = ids::resolve(&mut tx.store, &id, false)?;
        let current = std::fs::read_to_string(&tx.store.items[n].path)?;
        if current != orig_text {
            notices.append(&mut tx.changes.notices);
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
        notices.extend(reconcile(app, &orig_item, &mut item));
        tx.store.items[n] = item;
        tx.save(n, "edit")?;
        notices.append(&mut tx.changes.notices);
        let mut changes = tx.close(app);
        notices.append(&mut changes.notices);
        changes.notices = notices;
        return Ok(changes);
    }
}

/// Reconciles the heading checkbox with the status log after an edit. Returns a warning if
/// the checkbox disagreed with a change to the status log.
pub fn reconcile(app: &App, before: &Item, after: &mut Item) -> Option<String> {
    if after.kind() != Kind::Todo {
        return None;
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
        // `after` is a todo: the change cannot fail.
        status::apply(&app.cfg, after, Status::from_checkbox(c), app.clock.today).ok();
        return None;
    }
    let warning = (log_changed && after.checkbox_mismatch().is_some()).then(|| {
        format!(
            "warning: {}: the checkbox disagreed with the status log; doen rewrote the checkbox",
            after.id()
        )
    });
    after.sync_checkbox();
    warning
}

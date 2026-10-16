use crate::app::App;
use crate::cli::args::EditCmd;
use crate::commands::status;
use crate::editor::{self, edit_again, validate};
use crate::error::{Result, general, warn};
use crate::ids;
use crate::item::{Item, Kind, Status};

pub fn run(app: &App, c: EditCmd) -> Result<()> {
    let store = app.store()?;
    let n = ids::resolve(&store, &c.id, false)?;
    let id = store.items[n].id().to_string();
    edit_item(app, &id, false)
}

/// Edits the item with the ID `id`. The lock is held only while doen saves the file.
pub fn edit_item(app: &App, id: &str, quiet: bool) -> Result<()> {
    editor::require_terminal()?;
    let store = app.store()?;
    let n = ids::resolve(&store, id, false)?;
    let mut orig_item = store.items[n].clone();
    let mut orig_text = std::fs::read_to_string(&orig_item.path)?;
    let tmp = tempfile::Builder::new()
        .prefix(&format!("doen-{id}-"))
        .suffix(".md")
        .tempfile()?;
    std::fs::write(tmp.path(), &orig_text)?;
    loop {
        editor::open(app, tmp.path())?;
        let text = std::fs::read_to_string(tmp.path())?;
        if text == orig_text {
            eprintln!("doen: no changes");
            return Ok(());
        }
        let edited = match validate(&orig_item, &text) {
            Ok(i) => i,
            Err(e) => {
                edit_again(&format!("invalid frontmatter: {e}"))?;
                continue;
            }
        };
        let mut tx = app.begin()?;
        let n = ids::resolve(&tx.store, id, false)?;
        let current = std::fs::read_to_string(&tx.store.items[n].path)?;
        if current != orig_text {
            drop(tx);
            edit_again("the file changed while the editor was open")?;
            // The next save compares with the new version of the file.
            orig_text = current;
            orig_item = app
                .store()?
                .items
                .into_iter()
                .find(|i| i.id() == id)
                .ok_or_else(|| general("the item is gone"))?;
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

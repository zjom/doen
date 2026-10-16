//! `check [--fix]`.

use std::collections::BTreeMap;

use crate::app::App;
use crate::cli::args::CheckCmd;
use crate::error::{Error, Result};
use crate::ids;
use crate::store::{Store, filename, filename_id};

pub fn run(app: &App, c: CheckCmd) -> Result<()> {
    if c.fix {
        let mut tx = app.begin()?;
        let open = report(&mut tx.store, Some(&mut tx.changes))?;
        let lines = std::mem::take(&mut tx.changes.lines);
        for l in &lines {
            println!("{l}");
        }
        tx.changes
            .quiet_lines
            .extend(lines.iter().map(|l| format!("check --fix  {l}")));
        if app.cfg.git_autocommit {
            crate::git::commit(&app.root, &tx.changes);
        }
        return if open > 0 {
            Err(Error::Silent(1))
        } else {
            Ok(())
        };
    }
    let mut store = Store::open(&app.root, false)?;
    match report(&mut store, None)? {
        0 => Ok(()),
        _ => Err(Error::Silent(1)),
    }
}

/// Prints the problems and fixes them if `changes` is given. Returns the number of
/// problems that are not fixed.
fn report(store: &mut Store, mut changes: Option<&mut crate::store::Changes>) -> Result<usize> {
    let mut open = 0;
    let mut problem = |path: &std::path::Path, msg: String| {
        println!("{}: {msg}", path.display());
        open += 1;
    };
    for (path, e) in &store.invalid {
        problem(path, format!("invalid frontmatter: {e}"));
    }
    for item in &store.items {
        for t in &item.meta.bad_tags {
            problem(&item.path, format!("invalid tag '{t}'"));
        }
        if item
            .meta
            .heading
            .as_ref()
            .is_none_or(|h| h.title.is_empty())
        {
            problem(&item.path, "missing title".into());
        }
    }

    // Duplicate IDs: the item created most recently gets a new ID.
    let mut by_id: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (n, item) in store.items.iter().enumerate() {
        by_id.entry(item.id().to_string()).or_default().push(n);
    }
    for (id, mut group) in by_id.into_iter().filter(|(_, g)| g.len() > 1) {
        group.sort_by_key(|&n| store.items[n].meta.created);
        for &n in &group[1..] {
            let path = store.items[n].path.clone();
            match changes.as_deref_mut() {
                None => problem(&path, format!("duplicate id '{id}'")),
                Some(ch) => {
                    let new_id = ids::generate(store);
                    let mut item = store.items[n].clone();
                    item.set_id(&new_id);
                    store.write(&mut item, ch)?;
                    let name = item.path.file_name().unwrap().to_string_lossy().to_string();
                    let new_name = match (filename_id(&item.path), name.split_once('-')) {
                        (Some(old), Some((day, rest))) if old == id => {
                            format!("{day}-{new_id}{}", &rest[id.len()..])
                        }
                        _ => filename(&item),
                    };
                    store.rename(&mut item, &new_name, ch)?;
                    ch.lines.push(format!(
                        "{}: duplicate id '{id}': new id '{new_id}'",
                        path.display()
                    ));
                    store.items[n] = item;
                }
            }
        }
    }

    for n in 0..store.items.len() {
        let item = &store.items[n];
        if filename_id(&item.path) != Some(item.id()) {
            problem(
                &item.path,
                format!("the filename does not match the id '{}'", item.id()),
            );
        }
        let wrong_dir = item.place != item.meta.expected_place();
        let checkbox = item.checkbox_mismatch().is_some();
        let path = item.path.clone();
        let expected = item.meta.expected_place().as_str();
        match changes.as_deref_mut() {
            None => {
                if wrong_dir {
                    problem(
                        &path,
                        format!("wrong directory: the item belongs in {expected}"),
                    );
                }
                if checkbox {
                    problem(&path, "the checkbox disagrees with the status log".into());
                }
            }
            Some(ch) if wrong_dir || checkbox => {
                let mut item = store.items[n].clone();
                item.sync_checkbox();
                store.write(&mut item, ch)?;
                if wrong_dir {
                    ch.lines
                        .push(format!("{}: moved to {expected}", path.display()));
                }
                if checkbox {
                    ch.lines
                        .push(format!("{}: rewrote the checkbox", path.display()));
                }
                store.items[n] = item;
            }
            Some(_) => {}
        }
    }
    Ok(open)
}

use std::collections::BTreeMap;
use std::path::Path;

use crate::app::App;
use crate::cli::args::CheckCmd;
use crate::error::{Error, Result};
use crate::ids;
use crate::store::{Change, Changes, Store, filename, filename_id};

pub fn run(app: &App, c: CheckCmd) -> Result<()> {
    let unfixed = if c.fix {
        let mut tx = app.begin()?;
        let unfixed = report(&mut tx.store, Some(&mut tx.changes))?;
        tx.finish(app)?;
        unfixed
    } else {
        report(&mut Store::open(&app.root, false)?, None)?
    };
    match unfixed {
        0 => Ok(()),
        _ => Err(Error::Silent(1)),
    }
}

fn fixed(store: &Store, n: usize, ch: &mut Changes, path: &Path, msg: &str) {
    println!("{}: {msg}", path.display());
    ch.quiet.push(Change::new("check --fix", &store.items[n]));
}

/// Prints the problems and fixes them if `changes` is given. Returns the number of
/// problems that are not fixed.
fn report(store: &mut Store, mut changes: Option<&mut Changes>) -> Result<usize> {
    let mut open = 0;
    let mut problem = |path: &Path, msg: String| {
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
        if !item.has_title() {
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
            let Some(ch) = changes.as_deref_mut() else {
                problem(&path, format!("duplicate id '{id}'"));
                continue;
            };
            let new_id = ids::generate(store);
            store.items[n].set_id(&new_id);
            store.write(n, ch)?;
            let item = &store.items[n];
            let name = item.path.file_name().unwrap().to_string_lossy().to_string();
            let new_name = match (filename_id(&item.path), name.split_once('-')) {
                (Some(old), Some((day, rest))) if old == id => {
                    format!("{day}-{new_id}{}", &rest[id.len()..])
                }
                _ => filename(item),
            };
            store.rename(n, &new_name, ch)?;
            fixed(
                store,
                n,
                ch,
                &path,
                &format!("duplicate id '{id}': new id '{new_id}'"),
            );
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
        let Some(ch) = changes.as_deref_mut() else {
            if wrong_dir {
                problem(
                    &path,
                    format!("wrong directory: the item belongs in {expected}"),
                );
            }
            if checkbox {
                problem(&path, "the checkbox disagrees with the status log".into());
            }
            continue;
        };
        if !wrong_dir && !checkbox {
            continue;
        }
        store.items[n].sync_checkbox();
        store.write(n, ch)?;
        if wrong_dir {
            fixed(store, n, ch, &path, &format!("moved to {expected}"));
        }
        if checkbox {
            fixed(store, n, ch, &path, "rewrote the checkbox");
        }
    }
    Ok(open)
}

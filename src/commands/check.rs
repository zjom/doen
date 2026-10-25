use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::app::Tx;
use crate::commands::Confirm;
use crate::error::{Error, Result};
use crate::ids;
use crate::item::{Item, Place};
use crate::store::{Change, Changes, Store, TMP_PREFIX, filename, filename_id};

/// `check`, and with `fix`, `check --fix`. The report lines go to `changes.report`. `confirm` is
/// asked before the temporary files are deleted; `Declined` keeps them.
pub fn run(tx: &mut Tx, fix: bool, confirm: &mut Confirm) -> Result<()> {
    tx.store.load_places(&Place::ALL, false);
    let changes = if fix { Some(&mut tx.changes) } else { None };
    let (lines, mut unfixed) = report(&mut tx.store, changes)?;
    tx.changes.report.extend(lines);
    let found = temp_files(&tx.store.root);
    if !found.is_empty() {
        for (path, title) in &found {
            let line = format!("{}: temporary file: {title}", path.display());
            tx.changes.report.push(line);
        }
        let paths: Vec<PathBuf> = found.into_iter().map(|(p, _)| p).collect();
        match confirm(tx, &[], &temp_question(paths.len())) {
            Ok(()) => tx.changes.report.extend(delete_temp_files(&paths)?),
            Err(Error::Declined(_)) => unfixed += paths.len(),
            Err(e) => return Err(e),
        }
    }
    match unfixed {
        0 => Ok(()),
        _ => Err(Error::Silent(1)),
    }
}

pub fn temp_question(n: usize) -> String {
    format!(
        "Delete {n} temporary file{}? An open editor may still use it.",
        if n == 1 { "" } else { "s" }
    )
}

fn fixed(
    store: &Store,
    n: usize,
    ch: &mut Changes,
    lines: &mut Vec<String>,
    path: &Path,
    msg: &str,
) {
    lines.push(format!("{}: {msg}", path.display()));
    ch.quiet.push(Change::new("check --fix", &store.items[n]));
}

/// The problems of the store, and the fixes if `changes` is given. Returns the lines of the
/// report and the number of problems that are not fixed.
pub fn report(
    store: &mut Store,
    mut changes: Option<&mut Changes>,
) -> Result<(Vec<String>, usize)> {
    let mut lines = vec![];
    let mut open = 0;
    let mut problem = |lines: &mut Vec<String>, path: &Path, msg: String| {
        lines.push(format!("{}: {msg}", path.display()));
        open += 1;
    };
    for (path, e) in &store.invalid {
        problem(&mut lines, path, format!("invalid frontmatter: {e}"));
    }
    for item in &store.items {
        for t in &item.meta.bad_tags {
            problem(&mut lines, &item.path, format!("invalid tag '{t}'"));
        }
        if !item.has_title() {
            problem(&mut lines, &item.path, "missing title".into());
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
                problem(&mut lines, &path, format!("duplicate id '{id}'"));
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
            let msg = format!("duplicate id '{id}': new id '{new_id}'");
            fixed(store, n, ch, &mut lines, &path, &msg);
        }
    }

    for n in 0..store.items.len() {
        let item = &store.items[n];
        if filename_id(&item.path) != Some(item.id()) {
            problem(
                &mut lines,
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
                    &mut lines,
                    &path,
                    format!("wrong directory: the item belongs in {expected}"),
                );
            }
            if checkbox {
                problem(
                    &mut lines,
                    &path,
                    "the checkbox disagrees with the status log".into(),
                );
            }
            continue;
        };
        if !wrong_dir && !checkbox {
            continue;
        }
        store.items[n].sync_checkbox();
        store.write(n, ch)?;
        if wrong_dir {
            fixed(
                store,
                n,
                ch,
                &mut lines,
                &path,
                &format!("moved to {expected}"),
            );
        }
        if checkbox {
            fixed(store, n, ch, &mut lines, &path, "rewrote the checkbox");
        }
    }
    Ok((lines, open))
}

/// The temporary files left behind by a crash or an open editor, with the titles of their items.
pub fn temp_files(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found = vec![];
    for place in Place::ALL {
        let Ok(entries) = std::fs::read_dir(root.join(place.dir())) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().starts_with(TMP_PREFIX)
                    && e.file_type().is_ok_and(|t| t.is_file())
            })
            .map(|e| e.path())
            .collect();
        paths.sort();
        for path in paths {
            let title = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| Item::parse(&path, place, &text).ok().flatten())
                .map_or("(untitled)".to_string(), |i| i.title().to_string());
            found.push((path, title));
        }
    }
    found
}

/// Deletes the files. Returns the lines of the report.
pub fn delete_temp_files(paths: &[PathBuf]) -> Result<Vec<String>> {
    let mut lines = vec![];
    for path in paths {
        std::fs::remove_file(path)?;
        lines.push(format!("{}: deleted", path.display()));
    }
    Ok(lines)
}

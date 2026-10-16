use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::item::Place;
use crate::store::{Store, filename_id};

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// Generates a random ID that is not used in the store (root, archive and trash).
pub fn generate(store: &Store) -> String {
    let len = if store.file_count() > 50_000 { 5 } else { 4 };
    loop {
        let id: String = (0..len)
            .map(|_| ALPHABET[rand::random_range(0..ALPHABET.len())] as char)
            .collect();
        if !store.has_id(&id) {
            return id;
        }
    }
}

/// Resolves an ID or a unique prefix of 2 or more characters to an item index.
/// Items in the trash resolve only if `trash` is true.
///
/// Candidates are found by the IDs in the filenames of all places, and only those files are
/// parsed. If no filename matches, all places are parsed, for files named after another ID.
pub fn resolve(store: &mut Store, arg: &str, trash: bool) -> Result<usize> {
    let arg = arg.to_ascii_lowercase();
    let places: &[Place] = if trash {
        &Place::ALL
    } else {
        &[Place::Root, Place::Archive]
    };
    let named: Vec<(PathBuf, Place)> = store
        .index
        .iter()
        .filter(|(p, place)| {
            places.contains(place)
                && filename_id(p)
                    .is_some_and(|id| id == arg || arg.len() >= 2 && id.starts_with(&arg))
        })
        .cloned()
        .collect();
    let loaded: Vec<usize> = named
        .iter()
        .filter_map(|(p, place)| store.load(p, *place, false))
        .collect();
    let found = matches(store, &arg, trash, loaded.into_iter());
    if !found.is_empty() {
        return pick(store, &arg, found);
    }
    store.load_places(places, false);
    let found = matches(store, &arg, trash, 0..store.items.len());
    pick(store, &arg, found)
}

/// The items among `candidates` with the ID `arg`, or else with the prefix `arg`.
fn matches(
    store: &Store,
    arg: &str,
    trash: bool,
    candidates: impl Iterator<Item = usize>,
) -> Vec<usize> {
    let candidates: Vec<usize> = candidates
        .filter(|&n| {
            let i = &store.items[n];
            trash || i.place != Place::Trash && i.meta.trashed.is_none()
        })
        .collect();
    let exact: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&n| store.items[n].id() == arg)
        .collect();
    if !exact.is_empty() || arg.len() < 2 {
        return exact;
    }
    candidates
        .into_iter()
        .filter(|&n| store.items[n].id().starts_with(arg))
        .collect()
}

fn pick(store: &Store, arg: &str, found: Vec<usize>) -> Result<usize> {
    match found.as_slice() {
        [n] => Ok(*n),
        [] => Err(Error::NotFound(format!("no item with the id '{arg}'"))),
        many => {
            let list: Vec<String> = many
                .iter()
                .map(|&n| format!("  {}", store.items[n].label()))
                .collect();
            Err(Error::NotFound(format!(
                "the id '{arg}' is ambiguous:\n{}",
                list.join("\n")
            )))
        }
    }
}

/// Resolves every ID before any change. Duplicate arguments resolve once.
pub fn resolve_all(store: &mut Store, args: &[String], trash: bool) -> Result<Vec<usize>> {
    let mut out = vec![];
    for a in args {
        let n = resolve(store, a, trash)?;
        if !out.contains(&n) {
            out.push(n);
        }
    }
    Ok(out)
}

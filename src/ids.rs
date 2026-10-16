//! ID generation and resolution.

use crate::error::{Error, Result};
use crate::item::Place;
use crate::store::Store;

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
pub fn resolve(store: &Store, arg: &str, trash: bool) -> Result<usize> {
    let arg = arg.to_ascii_lowercase();
    let candidates = || {
        store
            .items
            .iter()
            .enumerate()
            .filter(move |(_, i)| trash || i.place != Place::Trash && i.meta.trashed.is_none())
    };
    let exact: Vec<usize> = candidates()
        .filter(|(_, i)| i.id() == arg)
        .map(|(n, _)| n)
        .collect();
    let matches = if !exact.is_empty() {
        exact
    } else if arg.len() >= 2 {
        candidates()
            .filter(|(_, i)| i.id().starts_with(&arg))
            .map(|(n, _)| n)
            .collect()
    } else {
        vec![]
    };
    match matches.as_slice() {
        [n] => Ok(*n),
        [] => Err(Error::NotFound(format!("no item with the id '{arg}'"))),
        many => {
            let list: Vec<String> = many
                .iter()
                .map(|&n| format!("  {}  {}", store.items[n].id(), store.items[n].title()))
                .collect();
            Err(Error::NotFound(format!(
                "the id '{arg}' is ambiguous:\n{}",
                list.join("\n")
            )))
        }
    }
}

/// Resolves every ID before any change. Duplicate arguments resolve once.
pub fn resolve_all(store: &Store, args: &[String], trash: bool) -> Result<Vec<usize>> {
    let mut out = vec![];
    for a in args {
        let n = resolve(store, a, trash)?;
        if !out.contains(&n) {
            out.push(n);
        }
    }
    Ok(out)
}

pub mod check;
pub mod completion;
pub mod create;
pub mod edit;
pub mod init;
pub mod lifecycle;
pub mod list;
pub mod meta;
pub mod show;
pub mod status;
pub mod tags;

use std::io::Read;

use crate::error::{Error, Result, usage};
use crate::ids;
use crate::item::Kind;
use crate::store::Store;

/// Resolves every ID before any change. The argument `-` reads whitespace-separated IDs from
/// stdin. Duplicate IDs resolve once.
pub fn resolve_ids(store: &mut Store, args: &[String], trash: bool) -> Result<Vec<usize>> {
    let mut expanded = vec![];
    let mut stdin_read = false;
    for a in args {
        if a != "-" {
            expanded.push(a.clone());
        } else if !stdin_read {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            expanded.extend(s.split_whitespace().map(String::from));
            stdin_read = true;
        }
    }
    if expanded.is_empty() {
        return Err(usage("no IDs given"));
    }
    let mut out = vec![];
    for a in &expanded {
        let n = ids::resolve(store, a, trash)?;
        if !out.contains(&n) {
            out.push(n);
        }
    }
    Ok(out)
}

pub fn require_todos(store: &Store, idx: &[usize], what: &str) -> Result<()> {
    match idx
        .iter()
        .map(|&n| &store.items[n])
        .find(|i| i.kind() == Kind::Note)
    {
        Some(i) => Err(only_todos(i.id(), what)),
        None => Ok(()),
    }
}

/// The error of a todo-only change, as `a priority`, to the note `id`.
pub fn only_todos(id: &str, what: &str) -> Error {
    usage(format!("{id} is a note: only todos have {what}"))
}

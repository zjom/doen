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

use crate::error::{Result, usage};
use crate::ids;
use crate::item::Item;
use crate::store::Store;

/// Resolves ID arguments. The argument `-` reads whitespace-separated IDs from stdin.
pub fn resolve_ids(store: &Store, args: &[String], trash: bool) -> Result<Vec<usize>> {
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
    ids::resolve_all(store, &expanded, trash)
}

pub fn item_line(item: &Item) -> String {
    format!("{}  {}", item.id(), item.title())
}

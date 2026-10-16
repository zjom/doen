//! The store: scanning, atomic writes, moves, the lock and trash purge.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{Result, general, warn};
use crate::item::{Item, Place};

pub struct Store {
    pub root: PathBuf,
    pub items: Vec<Item>,
    /// Files that look like doen items but cannot be parsed.
    pub invalid: Vec<(PathBuf, String)>,
}

/// One change to one item: an output line `verb  id  title` and a line of the commit message.
pub struct Change {
    pub verb: String,
    pub id: String,
    pub title: String,
}

impl Change {
    pub fn new(verb: impl Into<String>, item: &Item) -> Change {
        Change {
            verb: verb.into(),
            id: item.id().into(),
            title: item.title().into(),
        }
    }

    pub fn line(&self) -> String {
        format!("{}  {}  {}", self.verb, self.id, self.title)
    }
}

#[derive(Default)]
pub struct Changes {
    /// Printed on stdout and committed.
    pub shown: Vec<Change>,
    /// Committed only.
    pub quiet: Vec<Change>,
    pub paths: Vec<PathBuf>,
}

impl Changes {
    pub fn touch(&mut self, p: &Path) {
        if !self.paths.iter().any(|x| x == p) {
            self.paths.push(p.to_path_buf());
        }
    }

    pub fn hush(&mut self) {
        let shown = std::mem::take(&mut self.shown);
        self.quiet.extend(shown);
    }
}

/// The ID part of a filename `<YYYYMMDD>-<id>-<slug>.md`.
pub fn filename_id(path: &Path) -> Option<&str> {
    let stem = path.file_stem()?.to_str()?;
    stem.split('-').nth(1)
}

pub fn filename(item: &Item) -> String {
    let day = item.meta.created.format("%Y%m%d");
    match slug(item.title()) {
        s if s.is_empty() => format!("{day}-{}.md", item.id()),
        s => format!("{day}-{}-{s}.md", item.id()),
    }
}

/// ASCII, lowercase, words joined by `-`, at most 40 bytes, cut at a word boundary if possible.
fn slug(title: &str) -> String {
    let ascii = deunicode::deunicode(title).to_lowercase();
    let mut s = String::new();
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_matches('-');
    if s.len() <= 40 {
        return s.to_string();
    }
    let cut = if s.as_bytes()[40] == b'-' {
        40
    } else {
        s[..40].rfind('-').unwrap_or(40)
    };
    s[..cut].trim_end_matches('-').to_string()
}

pub fn require_root(root: &Path) -> Result<()> {
    if root.is_dir() {
        Ok(())
    } else {
        Err(general(format!(
            "no store at {}. Run 'doen init' to create one.",
            root.display()
        )))
    }
}

impl Store {
    pub fn open(root: &Path, warn_invalid: bool) -> Result<Store> {
        require_root(root)?;
        let mut store = Store {
            root: root.to_path_buf(),
            items: vec![],
            invalid: vec![],
        };
        for place in [Place::Root, Place::Archive, Place::Trash] {
            store.scan(place, warn_invalid)?;
        }
        Ok(store)
    }

    fn scan(&mut self, place: Place, warn_invalid: bool) -> Result<()> {
        let dir = self.root.join(place.dir());
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(general(format!("{}: {e}", dir.display()))),
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".md") && !name.starts_with(".doen-tmp-") && p.is_file()
            })
            .collect();
        paths.sort();
        for path in paths {
            let text = match fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    if warn_invalid {
                        warn(format!("{}: {e}", path.display()));
                    }
                    self.invalid.push((path, e.to_string()));
                    continue;
                }
            };
            match Item::parse(&path, place, &text) {
                Ok(Some(item)) => {
                    if warn_invalid && filename_id(&path) != Some(item.id()) {
                        warn(format!(
                            "{}: the filename does not match the id '{}'",
                            path.display(),
                            item.id()
                        ));
                    }
                    self.items.push(item);
                }
                Ok(None) => {}
                Err(e) => {
                    if warn_invalid {
                        warn(format!("skipped {}: {e}", path.display()));
                    }
                    self.invalid.push((path, e));
                }
            }
        }
        Ok(())
    }

    /// The number of items and invalid files, for the choice of the ID length.
    pub fn file_count(&self) -> usize {
        self.items.len() + self.invalid.len()
    }

    pub fn has_id(&self, id: &str) -> bool {
        self.items.iter().any(|i| i.id() == id)
    }

    /// Writes an item atomically and moves the file to match the frontmatter.
    pub fn write(&mut self, n: usize, changes: &mut Changes) -> Result<()> {
        let item = &mut self.items[n];
        let place = item.meta.expected_place();
        let dir = self.root.join(place.dir());
        fs::create_dir_all(&dir)?;
        let name = item
            .path
            .file_name()
            .ok_or_else(|| general("invalid path"))?;
        let target = dir.join(name);
        if target != item.path && target.exists() {
            return Err(general(format!(
                "cannot move to {}: the file exists",
                target.display()
            )));
        }
        atomic_write(&self.root, &target, &item.render())?;
        changes.touch(&target);
        if target != item.path {
            if item.path.exists() {
                fs::remove_file(&item.path)?;
            }
            changes.touch(&item.path);
        }
        item.path = target;
        item.place = place;
        Ok(())
    }

    /// Renames an item file inside its directory.
    pub fn rename(&mut self, n: usize, new_name: &str, changes: &mut Changes) -> Result<()> {
        let item = &mut self.items[n];
        let target = item.path.with_file_name(new_name);
        if target.exists() {
            return Err(general(format!(
                "cannot rename to {}: the file exists",
                target.display()
            )));
        }
        fs::rename(&item.path, &target)?;
        changes.touch(&item.path);
        changes.touch(&target);
        item.path = target;
        Ok(())
    }

    /// Deletes the items whose `trashed` date is more than `trash_days` days in the past.
    pub fn purge_trash(
        &mut self,
        today: chrono::NaiveDate,
        trash_days: i64,
        changes: &mut Changes,
    ) -> Result<Vec<Item>> {
        if trash_days <= 0 {
            return Ok(vec![]);
        }
        let (purged, keep) = std::mem::take(&mut self.items).into_iter().partition(|i| {
            i.meta
                .trashed
                .is_some_and(|t| (today - t).num_days() > trash_days)
        });
        self.items = keep;
        for item in &purged {
            delete(item, changes)?;
        }
        Ok(purged)
    }
}

pub fn delete(item: &Item, changes: &mut Changes) -> Result<()> {
    fs::remove_file(&item.path)?;
    changes.touch(&item.path);
    Ok(())
}

/// Writes through a temporary file in `root`, so that a reader never sees a partial file.
fn atomic_write(root: &Path, target: &Path, content: &str) -> Result<()> {
    let mut tmp = tempfile::Builder::new()
        .prefix(".doen-tmp-")
        .tempfile_in(root)?;
    tmp.write_all(content.as_bytes())?;
    tmp.as_file().sync_all()?;
    tmp.persist(target)
        .map_err(|e| general(format!("{}: {}", target.display(), e.error)))?;
    Ok(())
}

/// An exclusive lock on `<store>/.doen.lock`.
pub struct Lock {
    _file: File,
}

impl Lock {
    pub fn acquire(root: &Path) -> Result<Lock> {
        let path = root.join(".doen.lock");
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Lock { _file: file }),
                Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(fs::TryLockError::WouldBlock) => {
                    return Err(general("another doen process holds the lock on the store"));
                }
                Err(fs::TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs() {
        assert_eq!(slug("Call bank!"), "call-bank");
        assert_eq!(slug("Café über straße"), "cafe-uber-strasse");
        assert_eq!(slug("!!!"), "");
        assert_eq!(
            slug("a very long title that goes on and on beyond forty"),
            "a-very-long-title-that-goes-on-and-on"
        );
        assert_eq!(slug(&"x".repeat(50)), "x".repeat(40));
    }
}

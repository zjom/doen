use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{Result, general, warn};
use crate::item::{Item, Place};

pub const TMP_PREFIX: &str = ".doen-tmp-";

pub struct Store {
    pub root: PathBuf,
    pub items: Vec<Item>,
    /// Files that look like doen items but cannot be parsed.
    pub invalid: Vec<(PathBuf, String)>,
    /// Every file that may be an item, in all places, parsed or not.
    pub index: Vec<(PathBuf, Place)>,
    /// The files of the index that `load` has read.
    seen: HashSet<PathBuf>,
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
    /// Messages for stderr, without the `doen: ` prefix.
    pub notices: Vec<String>,
    /// The lines of a report, as `check` prints them on stdout.
    pub report: Vec<String>,
}

impl Changes {
    pub fn notice(&mut self, msg: impl Into<String>) {
        self.notices.push(msg.into());
    }

    pub fn warn(&mut self, msg: impl std::fmt::Display) {
        self.notices.push(format!("warning: {msg}"));
    }

    pub fn touch(&mut self, p: &Path) {
        if !self.paths.iter().any(|x| x == p) {
            self.paths.push(p.to_path_buf());
        }
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

fn mismatch(path: &Path, id: &str) -> String {
    format!(
        "{}: the filename does not match the id '{id}'",
        path.display()
    )
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
    /// Lists every folder, but parses only the files in `places`. Other items are parsed on
    /// demand by `load`, for example when an ID resolves to them.
    pub fn open(root: &Path, places: &[Place], warn_invalid: bool) -> Result<Store> {
        require_root(root)?;
        let mut store = Store {
            root: root.to_path_buf(),
            items: vec![],
            invalid: vec![],
            index: vec![],
            seen: HashSet::new(),
        };
        for place in Place::ALL {
            store.list(place)?;
        }
        store.load_places(places, warn_invalid);
        Ok(store)
    }

    fn list(&mut self, place: Place) -> Result<()> {
        let dir = self.root.join(place.dir());
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(general(format!("{}: {e}", dir.display()))),
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_str().unwrap_or("");
                name.ends_with(".md")
                    && !name.starts_with(TMP_PREFIX)
                    && match e.file_type() {
                        // Follows symlinks, without the cost of a `stat` for plain files.
                        Ok(t) if t.is_symlink() => e.path().is_file(),
                        Ok(t) => t.is_file(),
                        Err(_) => false,
                    }
            })
            .map(|e| e.path())
            .collect();
        paths.sort();
        self.index.extend(paths.into_iter().map(|p| (p, place)));
        Ok(())
    }

    /// Parses one file of the index. Returns the item index, or `None` if the file is not an
    /// item or cannot be parsed. A file that is loaded already is not parsed again.
    pub fn load(&mut self, path: &Path, place: Place, warn_invalid: bool) -> Option<usize> {
        if !self.seen.insert(path.to_path_buf()) {
            return self.items.iter().position(|i| i.path == path);
        }
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                if warn_invalid {
                    warn(format!("{}: {e}", path.display()));
                }
                self.invalid.push((path.to_path_buf(), e.to_string()));
                return None;
            }
        };
        match Item::parse(path, place, &text) {
            Ok(Some(item)) => {
                if warn_invalid && filename_id(path) != Some(item.id()) {
                    warn(mismatch(path, item.id()));
                }
                self.items.push(item);
                Some(self.items.len() - 1)
            }
            Ok(None) => None,
            Err(e) => {
                if warn_invalid {
                    warn(format!("skipped {}: {e}", path.display()));
                }
                self.invalid.push((path.to_path_buf(), e));
                None
            }
        }
    }

    /// The warnings for the files parsed so far: the files that cannot be parsed and the
    /// filenames that do not match the ID. `load` prints them if `warn_invalid` is set.
    pub fn load_warnings(&self) -> Vec<String> {
        let invalid = self
            .invalid
            .iter()
            .map(|(p, e)| format!("skipped {}: {e}", p.display()));
        let mismatched = (self.items.iter())
            .filter(|i| filename_id(&i.path) != Some(i.id()))
            .map(|i| mismatch(&i.path, i.id()));
        invalid.chain(mismatched).collect()
    }

    /// Parses every file in `places` that is not loaded yet.
    pub fn load_places(&mut self, places: &[Place], warn_invalid: bool) {
        for n in 0..self.index.len() {
            let (path, place) = self.index[n].clone();
            if places.contains(&place) && !self.seen.contains(&path) {
                self.load(&path, place, warn_invalid);
            }
        }
    }

    /// Whether an item has the ID, by filename or, for the parsed items, by frontmatter.
    pub fn has_id(&self, id: &str) -> bool {
        self.index.iter().any(|(p, _)| filename_id(p) == Some(id))
            || self.items.iter().any(|i| i.id() == id)
    }

    /// Writes an item atomically and moves the file to match the frontmatter. A move writes the
    /// file in place, then renames it: after a crash, the file is in one place only.
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
        let text = item.render();
        if target != item.path && item.path.exists() {
            atomic_write(&self.root, &item.path, &text)?;
            match fs::rename(&item.path, &target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                    atomic_write(&self.root, &target, &text)?;
                    fs::remove_file(&item.path)?;
                }
                Err(e) => return Err(general(format!("{}: {e}", target.display()))),
            }
        } else {
            atomic_write(&self.root, &target, &text)?;
        }
        changes.touch(&target);
        if target != item.path {
            changes.touch(&item.path);
        }
        item.path = target;
        item.place = place;
        Ok(())
    }

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
        .prefix(TMP_PREFIX)
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
    use super::{Store, slug};
    use crate::item::Place;

    #[test]
    fn index_without_parsing() {
        let dir = tempfile::tempdir().unwrap();
        // Invalid items: parsing one adds it to `invalid`.
        let bad = "---\nid: \"x\"\nkind: what\n---\n# x\n";
        for (sub, id) in [("", "aaaa"), (".archive", "bbbb"), (".trash", "cccc")] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::fs::write(
                dir.path().join(sub).join(format!("20261016-{id}-x.md")),
                bad,
            )
            .unwrap();
        }
        let store = Store::open(dir.path(), &[], false).unwrap();
        assert!(store.items.is_empty() && store.invalid.is_empty());
        assert_eq!(store.index.len(), 3);
        assert!(store.has_id("bbbb") && store.has_id("cccc") && !store.has_id("dddd"));
        let store = Store::open(dir.path(), &[Place::Root], false).unwrap();
        assert!(store.invalid.len() == 1 && store.has_id("aaaa"));
    }

    #[test]
    fn move_on_write() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::testing::app(dir.path());
        let id = crate::testing::create(&app, crate::item::Kind::Todo, "x");
        let mut store = Store::open(dir.path(), &[Place::Root], false).unwrap();
        let old = store.items[0].path.clone();
        store.items[0].archive(app.clock.today);
        let mut changes = super::Changes::default();
        store.write(0, &mut changes).unwrap();
        let new = &store.items[0].path;
        assert_eq!(
            new,
            &dir.path().join(".archive").join(old.file_name().unwrap())
        );
        assert!(!old.exists());
        assert!(
            std::fs::read_to_string(new)
                .unwrap()
                .contains("archived: 2026-10-16")
        );
        let files = |d: &str| {
            std::fs::read_dir(dir.path().join(d))
                .unwrap()
                .filter(|e| {
                    e.as_ref()
                        .unwrap()
                        .path()
                        .extension()
                        .is_some_and(|x| x == "md")
                })
                .count()
        };
        assert_eq!((files(""), files(".archive")), (0, 1));
        assert_eq!(changes.paths, [new.clone(), old]);
        assert_eq!(store.items[0].id(), id);
    }

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

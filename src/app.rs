//! The context of one command run, and the transaction of a mutating command.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::config::{ColorMode, Config, store_path};
use crate::dates::{self, Clock};
use crate::error::{Error, Result};
use crate::git;
use crate::item::Place;
use crate::store::{self, Change, Changes, Lock, Store, require_root};
use crate::term;

pub struct App {
    pub root: PathBuf,
    pub cfg: Config,
    pub clock: Clock,
    pub yes: bool,
    pub color: bool,
}

impl App {
    pub fn new(dir: Option<&Path>, color: Option<ColorMode>, yes: bool) -> Result<App> {
        let root = store_path(dir);
        let cfg = Config::load(&root)?;
        let color = match color.or(cfg.color).unwrap_or(ColorMode::Auto) {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => term::stdout_is_terminal(),
        };
        let clock = Clock::system(cfg.week_start);
        Ok(App {
            root,
            cfg,
            clock,
            yes,
            color,
        })
    }

    /// Opens the store and parses the items in `places`. Other items are parsed when an ID
    /// resolves to them.
    pub fn store(&self, places: &[Place]) -> Result<Store> {
        Store::open(&self.root, places, true)
    }

    /// Takes the lock, loads the store and purges old trash.
    pub fn begin(&self) -> Result<Tx> {
        self.begin_with(&[])
    }

    /// Like `begin`, and parses the items in `places` too.
    pub fn begin_with(&self, places: &[Place]) -> Result<Tx> {
        require_root(&self.root)?;
        let lock = Lock::acquire(&self.root)?;
        let mut store = self.store(&[Place::Trash])?;
        store.load_places(places, true);
        let mut changes = Changes::default();
        for item in store.purge_trash(self.clock.today, self.cfg.trash_days, &mut changes)? {
            eprintln!("doen: deleted permanently from the trash: {}", item.label());
            changes.quiet.push(Change::new("purge", &item));
        }
        Ok(Tx {
            store,
            changes,
            _lock: lock,
        })
    }

    /// Runs a mutating command in a transaction. Prints the notices on stderr, whether `f`
    /// fails or not. If `f` succeeds, prints the output lines on stdout and makes the Git commit.
    pub fn run_tx<T>(&self, places: &[Place], f: impl FnOnce(&mut Tx) -> Result<T>) -> Result<T> {
        let mut tx = self.begin_with(places)?;
        let r = f(&mut tx);
        print_notices(&mut tx.changes.notices);
        let v = r?;
        let mut changes = tx.close(self);
        for c in &changes.shown {
            println!("{}", c.line());
        }
        print_notices(&mut changes.notices);
        Ok(v)
    }

    /// Lists the items on stderr and asks for confirmation on the terminal. `-y` skips the prompt.
    /// The notices so far are printed first.
    pub fn confirm(&self, tx: &mut Tx, idx: &[usize], question: &str) -> Result<()> {
        print_notices(&mut tx.changes.notices);
        for &n in idx {
            eprintln!("  {}", tx.store.items[n].label());
        }
        if self.yes {
            return Ok(());
        }
        let yes = term::confirm(question).map_err(|_| {
            Error::Declined("confirmation needed, but no terminal is available (use -y)".into())
        })?;
        if yes {
            Ok(())
        } else {
            Err(Error::Declined("cancelled".into()))
        }
    }

    pub fn show_date(&self, d: NaiveDate, overdue: bool) -> String {
        dates::display(
            d,
            &self.clock,
            self.cfg.relative_days,
            &self.cfg.date_format,
            overdue,
        )
    }

    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.color && !s.is_empty() {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

fn print_notices(notices: &mut Vec<String>) {
    for m in notices.drain(..) {
        eprintln!("doen: {m}");
    }
}

pub struct Tx {
    pub store: Store,
    pub changes: Changes,
    _lock: Lock,
}

impl Tx {
    /// Writes an item and records the output line `verb  id  title`.
    pub fn save(&mut self, n: usize, verb: &str) -> Result<()> {
        self.store.write(n, &mut self.changes)?;
        self.changes
            .shown
            .push(Change::new(verb, &self.store.items[n]));
        Ok(())
    }

    pub fn delete(&mut self, idx: &[usize]) -> Result<()> {
        for &n in idx {
            let item = &self.store.items[n];
            store::delete(item, &mut self.changes)?;
            self.changes.shown.push(Change::new("deleted", item));
        }
        Ok(())
    }

    /// Prints the output lines and the notices, and makes the Git commit.
    pub fn finish(self, app: &App) -> Result<()> {
        let mut changes = self.close(app);
        for c in &changes.shown {
            println!("{}", c.line());
        }
        print_notices(&mut changes.notices);
        Ok(())
    }

    /// Makes the Git commit and returns the changes, with a failure of the commit as a notice.
    /// Prints nothing.
    pub fn close(mut self, app: &App) -> Changes {
        if app.cfg.git_autocommit
            && let Some(w) = git::commit(&app.root, &self.changes)
        {
            self.changes.warn(w);
        }
        self.changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_failure_is_a_notice() {
        let dir = tempfile::tempdir().unwrap();
        let app = App {
            root: dir.path().into(),
            cfg: Config {
                git_autocommit: true,
                ..Config::default()
            },
            clock: Clock::at(chrono::Local::now().fixed_offset(), chrono::Weekday::Mon),
            yes: false,
            color: false,
        };
        let mut tx = app.begin().unwrap();
        let p = dir.path().join("x.md");
        std::fs::write(&p, "x").unwrap();
        tx.changes.touch(&p);
        let changes = tx.close(&app);
        assert_eq!(
            changes.notices,
            ["warning: no Git commit: the store is not in a Git work tree"]
        );
    }
}

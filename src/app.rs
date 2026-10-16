//! The context of one command run, and the transaction of a mutating command.

use std::path::{Path, PathBuf};

use crate::config::{ColorMode, Config, store_path};
use crate::dates::Clock;
use crate::error::{Error, Result};
use crate::git;
use crate::item::Item;
use crate::store::{Change, Changes, Lock, Store, require_root};
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

    pub fn store(&self) -> Result<Store> {
        Store::open(&self.root, true)
    }

    /// Takes the lock, loads the store and purges old trash.
    pub fn begin(&self) -> Result<Tx> {
        require_root(&self.root)?;
        let lock = Lock::acquire(&self.root)?;
        let mut store = self.store()?;
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

    /// Lists the items on stderr and asks for confirmation on the terminal. `-y` skips the prompt.
    pub fn confirm(&self, items: &[&Item], question: &str) -> Result<()> {
        for i in items {
            eprintln!("  {}", i.label());
        }
        if self.yes {
            return Ok(());
        }
        let answer = term::ask(&format!("{question} [y/N] ")).map_err(|_| {
            Error::Declined("confirmation needed, but no terminal is available (use -y)".into())
        })?;
        if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            Ok(())
        } else {
            Err(Error::Declined("cancelled".into()))
        }
    }

    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.color && !s.is_empty() {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
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

    /// Prints the output lines and makes the Git commit.
    pub fn finish(self, app: &App) -> Result<()> {
        for c in &self.changes.shown {
            println!("{}", c.line());
        }
        if app.cfg.git_autocommit {
            git::commit(&app.root, &self.changes);
        }
        Ok(())
    }
}

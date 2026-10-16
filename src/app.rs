//! The context of one command run, and the transaction of a mutating command.

use std::path::{Path, PathBuf};

use crate::config::{ColorMode, Config, store_path};
use crate::dates::Clock;
use crate::error::{Error, Result};
use crate::git;
use crate::store::{Changes, Lock, Store, require_root};
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

    /// Loads the store for a read-only command.
    pub fn store(&self) -> Result<Store> {
        Store::open(&self.root, true)
    }

    /// Takes the lock, loads the store and purges old trash.
    pub fn begin(&self) -> Result<Tx> {
        require_root(&self.root)?;
        let lock = Lock::acquire(&self.root)?;
        let mut store = self.store()?;
        let mut changes = Changes::default();
        store.purge_trash(self.clock.today, self.cfg.trash_days, &mut changes)?;
        Ok(Tx {
            store,
            changes,
            _lock: lock,
        })
    }

    /// Asks for confirmation on the terminal. `-y` skips the prompt.
    pub fn confirm(&self, question: &str) -> Result<()> {
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
        let mut item = self.store.items[n].clone();
        self.store.write(&mut item, &mut self.changes)?;
        self.store.items[n] = item;
        let item = &self.store.items[n];
        self.changes
            .lines
            .push(format!("{verb}  {}  {}", item.id(), item.title()));
        Ok(())
    }

    /// Prints the output lines and makes the Git commit.
    pub fn finish(self, app: &App) -> Result<()> {
        for line in &self.changes.lines {
            println!("{line}");
        }
        if app.cfg.git_autocommit {
            git::commit(&app.root, &self.changes);
        }
        Ok(())
    }
}

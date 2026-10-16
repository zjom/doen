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

use std::ffi::OsString;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::PathBuf;

use crate::cli::args::GlobalArgs;
use crate::config::{ColorMode, Config, store_path};
use crate::dates::Clock;
use crate::error::{Error, Result, usage};
use crate::git;
use crate::ids;
use crate::item::Item;
use crate::store::{Changes, Lock, Store};

/// The context of one command run.
pub struct App {
    pub root: PathBuf,
    pub cfg: Config,
    pub clock: Clock,
    pub yes: bool,
    pub color: bool,
}

impl App {
    pub fn new(g: &GlobalArgs) -> Result<App> {
        let root = store_path(g.dir.as_deref());
        let cfg = Config::load(&root)?;
        let mode = match g.color.as_deref() {
            Some(c) => {
                ColorMode::parse(c).ok_or_else(|| usage(format!("invalid --color value '{c}'")))?
            }
            None => cfg.color.unwrap_or(ColorMode::Auto),
        };
        let color = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => std::io::stdout().is_terminal(),
        };
        let clock = Clock::system(cfg.week_start);
        Ok(App {
            root,
            cfg,
            clock,
            yes: g.yes,
            color,
        })
    }

    /// Loads the store for a read-only command.
    pub fn store(&self) -> Result<Store> {
        Store::open(&self.root, true)
    }

    /// Starts a mutating command: takes the lock, loads the store and purges old trash.
    pub fn begin(&self) -> Result<Tx> {
        Store::open(&self.root, false)?;
        let lock = Lock::acquire(&self.root)?;
        let mut store = Store::open(&self.root, true)?;
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
        let answer = ask(&format!("{question} [y/N] ")).map_err(|_| {
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

/// Prints a prompt on stderr and reads one line from the terminal.
pub fn ask(prompt: &str) -> std::io::Result<String> {
    let tty = std::fs::File::options()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    BufReader::new(tty).read_line(&mut line)?;
    Ok(line)
}

/// Whether stdout is a terminal. The hidden `DOEN_ASSUME_TTY` variable is a test hook.
pub fn stdout_is_terminal() -> bool {
    std::env::var_os("DOEN_ASSUME_TTY").is_some() || std::io::stdout().is_terminal()
}

/// Whether a terminal is available for the editor. The hidden `DOEN_ASSUME_TTY` variable is a
/// test hook.
pub fn has_terminal() -> bool {
    std::env::var_os("DOEN_ASSUME_TTY").is_some()
        || std::fs::File::options()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .is_ok()
}

/// A mutating command in progress.
pub struct Tx {
    pub store: Store,
    pub changes: Changes,
    _lock: Lock,
}

impl Tx {
    /// Writes an item and records the output line `verb  id  title`.
    pub fn save(&mut self, n: usize, verb: &str) -> Result<()> {
        self.store_write(n)?;
        let item = &self.store.items[n];
        self.changes
            .lines
            .push(format!("{verb}  {}  {}", item.id(), item.title()));
        Ok(())
    }

    pub fn store_write(&mut self, n: usize) -> Result<()> {
        let mut item = self.store.items[n].clone();
        self.store.write(&mut item, &mut self.changes)?;
        self.store.items[n] = item;
        Ok(())
    }

    /// Prints the output lines and makes the Git commit.
    pub fn finish(self, app: &App) -> Result<()> {
        for line in &self.changes.lines {
            println!("{line}");
        }
        git::commit(app, &self.changes);
        Ok(())
    }
}

/// Expands the ID arguments. The argument `-` reads whitespace-separated IDs from stdin.
pub fn expand_ids(args: &[String]) -> Result<Vec<String>> {
    let mut out = vec![];
    let mut stdin_read = false;
    for a in args {
        if a == "-" {
            if !stdin_read {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                out.extend(s.split_whitespace().map(String::from));
                stdin_read = true;
            }
        } else {
            out.push(a.clone());
        }
    }
    if out.is_empty() {
        return Err(usage("no IDs given"));
    }
    Ok(out)
}

/// Resolves the ID arguments of a mutating command.
pub fn resolve(tx: &Tx, args: &[String], trash: bool) -> Result<Vec<usize>> {
    ids::resolve_all(&tx.store, &expand_ids(args)?, trash)
}

/// Whether an unknown command word is the ID of an item.
pub fn looks_like_id(pre: &[OsString], word: &str) -> bool {
    if word.len() < 2 || !word.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return false;
    }
    let mut g = GlobalArgs::default();
    let mut it = pre.iter().map(|a| a.to_string_lossy().to_string());
    while let Some(a) = it.next() {
        if a == "--dir" {
            g.dir = it.next().map(PathBuf::from);
        } else if let Some(d) = a.strip_prefix("--dir=") {
            g.dir = Some(PathBuf::from(d));
        }
    }
    let root = store_path(g.dir.as_deref());
    Store::open(&root, false).is_ok_and(|s| ids::resolve(&s, word, true).is_ok())
}

pub fn item_line(item: &Item) -> String {
    format!("{}  {}", item.id(), item.title())
}

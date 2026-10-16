//! The terminal. The hidden `DOEN_ASSUME_TTY` variable is a test hook that makes doen act as if
//! a terminal is available.

use std::fs::File;
use std::io::{BufRead, BufReader, IsTerminal, Write};

fn assume_tty() -> bool {
    std::env::var_os("DOEN_ASSUME_TTY").is_some()
}

fn open_tty() -> std::io::Result<File> {
    File::options().read(true).write(true).open("/dev/tty")
}

pub fn stdout_is_terminal() -> bool {
    assume_tty() || std::io::stdout().is_terminal()
}

pub fn has_terminal() -> bool {
    assume_tty() || open_tty().is_ok()
}

/// Prints a prompt on stderr and reads one line from the terminal.
pub fn ask(prompt: &str) -> std::io::Result<String> {
    let tty = open_tty()?;
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    BufReader::new(tty).read_line(&mut line)?;
    Ok(line)
}

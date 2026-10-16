use std::fmt;

/// An error that ends the command. Each variant maps to one exit code.
#[derive(Debug)]
pub enum Error {
    /// Exit code 1.
    General(String),
    /// Exit code 2: invalid option, invalid date, missing argument, unknown command,
    /// command on the wrong kind of item.
    Usage(String),
    /// Exit code 3: an ID is not found or is ambiguous.
    NotFound(String),
    /// Exit code 4: the user declined, or no terminal was available for a prompt.
    Declined(String),
    /// Exit with a code and print nothing more (the message was already printed).
    Silent(i32),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::General(_) => 1,
            Error::Usage(_) => 2,
            Error::NotFound(_) => 3,
            Error::Declined(_) => 4,
            Error::Silent(c) => *c,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::General(m) | Error::Usage(m) | Error::NotFound(m) | Error::Declined(m) => {
                f.write_str(m)
            }
            Error::Silent(_) => Ok(()),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::General(e.to_string())
    }
}

pub fn usage(msg: impl Into<String>) -> Error {
    Error::Usage(msg.into())
}

pub fn general(msg: impl Into<String>) -> Error {
    Error::General(msg.into())
}

/// Prints a warning on stderr.
pub fn warn(msg: impl fmt::Display) {
    eprintln!("doen: warning: {msg}");
}

use std::fmt;

#[derive(Debug)]
pub enum Error {
    General(String),
    Usage(String),
    NotFound(String),
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

pub fn warn(msg: impl fmt::Display) {
    eprintln!("doen: warning: {msg}");
}

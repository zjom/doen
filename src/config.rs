//! Layered configuration: CLI > `DOEN_*` env > store config > global config > general env > defaults.

use std::path::{Path, PathBuf};

use chrono::Weekday;
use clap::ValueEnum;

use crate::error::{Result, general, warn};
use crate::filter::SortField;
use crate::frontmatter::{Value, parse_yaml};
use crate::tags;

pub const KEYS: [&str; 13] = [
    "editor",
    "date_format",
    "week_start",
    "relative_days",
    "default_tags",
    "default_sort",
    "archive_closed",
    "trash_days",
    "git_autocommit",
    "color",
    "ls_header",
    "ls_footer",
    "templates_dir",
];

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub editor: Option<String>,
    pub date_format: String,
    pub week_start: Weekday,
    pub relative_days: i64,
    pub default_tags: Vec<String>,
    pub default_sort: Option<SortField>,
    pub archive_closed: bool,
    pub trash_days: i64,
    pub git_autocommit: bool,
    pub color: Option<ColorMode>,
    pub ls_header: bool,
    pub ls_footer: bool,
    pub templates_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            editor: None,
            date_format: "%y-%m-%d".into(),
            week_start: Weekday::Mon,
            relative_days: 7,
            default_tags: vec![],
            default_sort: None,
            archive_closed: true,
            trash_days: 30,
            git_autocommit: false,
            color: None,
            ls_header: true,
            ls_footer: true,
            templates_dir: None,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn xdg_config_dir() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join("doen"),
        None => home().join(".config").join("doen"),
    }
}

/// Expands a leading `~/` to `$HOME`.
fn expand_home(s: &str) -> PathBuf {
    match s.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if s == "~" => home(),
        None => PathBuf::from(s),
    }
}

pub fn global_config_path() -> PathBuf {
    match std::env::var_os("DOEN_CONFIG").filter(|v| !v.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => xdg_config_dir().join("config.yaml"),
    }
}

/// The store location: `--dir`, then `$DOEN_NOTES_DIR`, then `$HOME/doen/`.
pub fn store_path(dir: Option<&Path>) -> PathBuf {
    if let Some(d) = dir {
        return d.to_path_buf();
    }
    match std::env::var_os("DOEN_NOTES_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => home().join("doen"),
    }
}

fn read_file(path: &Path) -> Result<Vec<(String, Value)>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(general(format!("{}: {e}", path.display()))),
    };
    match parse_yaml(&text) {
        Ok(None) => Ok(vec![]),
        Ok(Some(Value::Map(m))) => Ok(m),
        Ok(Some(_)) => Err(general(format!(
            "{}: the config must be a mapping",
            path.display()
        ))),
        Err(e) => Err(general(format!("{}: {e}", path.display()))),
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

impl Config {
    pub fn load(store: &Path) -> Result<Config> {
        let mut cfg = Config {
            editor: std::env::var("VISUAL")
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| std::env::var("EDITOR").ok().filter(|v| !v.is_empty())),
            ..Config::default()
        };
        if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            cfg.color = Some(ColorMode::Never);
        }
        for src in [global_config_path(), store.join(".doen.yaml")] {
            for (k, v) in read_file(&src)? {
                cfg.apply(&k, &v)
                    .map_err(|e| general(format!("{}: {e}", src.display())))?;
            }
        }
        for key in KEYS {
            let var = format!("DOEN_{}", key.to_ascii_uppercase());
            if let Ok(v) = std::env::var(&var) {
                cfg.apply(key, &Value::Str(v))
                    .map_err(|e| general(format!("{var}: {e}")))?;
            }
        }
        Ok(cfg)
    }

    fn apply(&mut self, key: &str, v: &Value) -> std::result::Result<(), String> {
        if !KEYS.contains(&key) {
            warn(format!("unknown config key '{key}'"));
            return Ok(());
        }
        if key == "default_tags" {
            self.default_tags = tags::from_value(v)?;
            return Ok(());
        }
        let s = v
            .as_str()
            .ok_or_else(|| format!("'{key}' must be a single value"))?;
        let bad = || format!("invalid value for '{key}': '{s}'");
        let flag = || parse_bool(s).ok_or_else(bad);
        match key {
            "editor" => self.editor = Some(s.into()),
            "date_format" => self.date_format = s.into(),
            "week_start" => self.week_start = s.parse().map_err(|_| bad())?,
            "relative_days" => self.relative_days = s.parse().map_err(|_| bad())?,
            "trash_days" => self.trash_days = s.parse().map_err(|_| bad())?,
            "archive_closed" => self.archive_closed = flag()?,
            "git_autocommit" => self.git_autocommit = flag()?,
            "ls_header" => self.ls_header = flag()?,
            "ls_footer" => self.ls_footer = flag()?,
            "color" => self.color = Some(ColorMode::from_str(s, false).map_err(|_| bad())?),
            "templates_dir" if s.is_empty() => self.templates_dir = None,
            "templates_dir" => self.templates_dir = Some(expand_home(s)),
            "default_sort" if s.is_empty() => self.default_sort = None,
            "default_sort" => {
                self.default_sort = Some(SortField::from_str(s, false).map_err(|_| bad())?)
            }
            _ => unreachable!("every key in KEYS is handled"),
        }
        Ok(())
    }

    /// The global templates directory: `templates_dir`, or `$XDG_CONFIG_HOME/doen/templates/`.
    /// A relative `templates_dir` is relative to the store.
    pub fn templates_dir(&self, store: &Path) -> PathBuf {
        match &self.templates_dir {
            Some(d) => store.join(d),
            None => xdg_config_dir().join("templates"),
        }
    }

    pub fn editor_command(&self) -> String {
        self.editor.clone().unwrap_or_else(|| "vi".into())
    }
}

pub const DEFAULT_CONFIG: &str = "\
# doen config. Every key shows its default value.
# A key in <store>/.doen.yaml replaces the same key here.

# editor: vi              # default: $VISUAL, then $EDITOR, then vi
# date_format: \"%y-%m-%d\"
# week_start: mon
# relative_days: 7
# default_tags: []
# default_sort: due       # not set by default
# archive_closed: true
# trash_days: 30
# git_autocommit: false
# color: auto
# ls_header: true        # the banner above `doen ls`
# ls_footer: true        # the cheatsheet below `doen ls`
# templates_dir: ~/.config/doen/templates
";

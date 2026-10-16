//! Layered configuration: CLI > `DOEN_*` env > store config > global config > general env > defaults.

use std::path::{Path, PathBuf};

use chrono::Weekday;

use crate::dates::parse_weekday;
use crate::error::{Result, general, warn};
use crate::frontmatter::{Value, parse_yaml};
use crate::tags;

pub const KEYS: [&str; 10] = [
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
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn parse(s: &str) -> Option<ColorMode> {
        match s {
            "auto" => Some(ColorMode::Auto),
            "always" => Some(ColorMode::Always),
            "never" => Some(ColorMode::Never),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub editor: Option<String>,
    pub date_format: String,
    pub week_start: Weekday,
    pub relative_days: i64,
    pub default_tags: Vec<String>,
    pub default_sort: Option<String>,
    pub archive_closed: bool,
    pub trash_days: i64,
    pub git_autocommit: bool,
    pub color: Option<ColorMode>,
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
        for (src, entries) in [
            (global_config_path(), read_file(&global_config_path())?),
            (
                store.join(".doen.yaml"),
                read_file(&store.join(".doen.yaml"))?,
            ),
        ] {
            for (k, v) in entries {
                cfg.apply(&k, &v)
                    .map_err(|e| general(format!("{}: {e}", src.display())))?;
            }
        }
        for key in KEYS {
            if let Ok(v) = std::env::var(format!("DOEN_{}", key.to_ascii_uppercase())) {
                let value = if key == "default_tags" {
                    Value::Seq(v.split(',').map(|t| Value::Str(t.trim().into())).collect())
                } else {
                    Value::Str(v)
                };
                cfg.apply(key, &value)
                    .map_err(|e| general(format!("DOEN_{}: {e}", key.to_ascii_uppercase())))?;
            }
        }
        Ok(cfg)
    }

    fn apply(&mut self, key: &str, v: &Value) -> std::result::Result<(), String> {
        let s = || {
            v.as_str()
                .ok_or_else(|| format!("'{key}' must be a single value"))
        };
        let bad = |s: &str| format!("invalid value for '{key}': '{s}'");
        match key {
            "editor" => self.editor = Some(s()?.to_string()),
            "date_format" => self.date_format = s()?.to_string(),
            "week_start" => {
                self.week_start = parse_weekday(s()?).ok_or_else(|| bad(s().unwrap()))?
            }
            "relative_days" => self.relative_days = s()?.parse().map_err(|_| bad(s().unwrap()))?,
            "trash_days" => self.trash_days = s()?.parse().map_err(|_| bad(s().unwrap()))?,
            "archive_closed" => {
                self.archive_closed = parse_bool(s()?).ok_or_else(|| bad(s().unwrap()))?
            }
            "git_autocommit" => {
                self.git_autocommit = parse_bool(s()?).ok_or_else(|| bad(s().unwrap()))?
            }
            "color" => self.color = Some(ColorMode::parse(s()?).ok_or_else(|| bad(s().unwrap()))?),
            "default_sort" => {
                let f = s()?;
                if !f.is_empty() {
                    crate::filter::SortField::parse(f).ok_or_else(|| bad(f))?;
                    self.default_sort = Some(f.to_string());
                }
            }
            "default_tags" => {
                let list: Vec<&str> = match v {
                    Value::Seq(items) => items.iter().filter_map(Value::as_str).collect(),
                    Value::Str(s) if s.is_empty() => vec![],
                    Value::Str(s) => s.split(',').collect(),
                    Value::Map(_) => return Err(bad("{...}")),
                };
                self.default_tags = list
                    .iter()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| tags::normalize(t))
                    .collect::<std::result::Result<_, _>>()?;
            }
            _ => warn(format!("unknown config key '{key}'")),
        }
        Ok(())
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
";

use std::collections::BTreeSet;

use std::ffi::OsString;

use crate::app::App;
use crate::cli::args::{CompleteCmd, Shell};
use crate::cli::resolve::{Resolved, resolve};
use crate::cli::{command_word, dir_of};
use crate::complete::{Context, candidates};
use crate::error::{Result, usage};
use crate::item::Place;
use crate::store::Store;
use crate::template;

const BASH: &str = include_str!("../completion/doen.bash");
const ZSH: &str = include_str!("../completion/_doen");
const FISH: &str = include_str!("../completion/doen.fish");

pub fn print(shell: Shell) {
    print!(
        "{}",
        match shell {
            Shell::Bash => BASH,
            Shell::Zsh => ZSH,
            Shell::Fish => FISH,
        }
    );
}

pub const DATES: &str =
    "today yesterday tomorrow mon tue wed thu fri sat sun eow eom eoy +1d +2d +1w +1m none";

pub fn templates(app: &App) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for dir in template::dirs(app) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Some(n) = e.file_name().to_str().and_then(|n| n.strip_suffix(".md")) {
                names.insert(n.to_string());
            }
        }
    }
    names
}

/// `doen __complete line <words>... <word>`: the candidates for the last word, as
/// `value<TAB>help` lines. A `--dir` in the words selects the store.
fn line(app: &App, words: &[String]) -> Result<()> {
    let (word, words) = match words.split_last() {
        Some((w, rest)) => (w.as_str(), rest),
        None => ("", words),
    };
    let argv: Vec<OsString> = words.iter().map(OsString::from).collect();
    let i = command_word(&argv);
    let other;
    let app = match dir_of(&argv[..i]) {
        Some(dir) => {
            other = App::new(Some(&dir), None, false)?;
            &other
        }
        None => app,
    };
    let mut places = vec![Place::Root, Place::Archive];
    if words
        .get(i)
        .is_some_and(|w| resolve(w) == Resolved::Command("restore".into()))
    {
        places.push(Place::Trash);
    }
    let store = Store::open(&app.root, &places, false).unwrap_or_else(|_| Store::empty(&app.root));
    for c in candidates(Context::Shell, words, word, &store, app) {
        match c.help.lines().next() {
            Some(help) if !help.is_empty() => println!("{}\t{help}", c.value),
            _ => println!("{}", c.value),
        }
    }
    Ok(())
}

pub fn data(app: &App, c: CompleteCmd) -> Result<()> {
    match c.what.as_str() {
        "line" => return line(app, &c.words),
        "ids" => {
            let store = app.store(&[Place::Root, Place::Archive])?;
            for i in &store.items {
                println!("{}\t{}", i.id(), i.title());
            }
        }
        "tags" => {
            let store = app.store(&[Place::Root, Place::Archive])?;
            let tags: BTreeSet<&str> = store
                .items
                .iter()
                .flat_map(|i| i.meta.tags.iter().map(String::as_str))
                .collect();
            for t in tags {
                println!("{t}");
            }
        }
        "templates" => {
            for n in templates(app) {
                println!("{n}");
            }
        }
        "dates" => {
            for d in DATES.split(' ') {
                println!("{d}");
            }
        }
        what => return Err(usage(format!("unknown completion data '{what}'"))),
    }
    Ok(())
}

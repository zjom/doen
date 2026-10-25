use std::collections::BTreeSet;

use crate::app::App;
use crate::cli::args::Shell;
use crate::error::{Result, usage};
use crate::item::Place;
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

pub fn data(app: &App, what: &str) -> Result<()> {
    match what {
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
        _ => return Err(usage(format!("unknown completion data '{what}'"))),
    }
    Ok(())
}

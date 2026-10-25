pub mod args;
pub mod resolve;

use std::ffi::OsString;

use clap::{CommandFactory, Parser};

use crate::app::App;
use crate::commands::{completion, create, dispatch, edit, init, list, show, tags};
use crate::config::store_path;
use crate::error::{Error, Result, usage};
use crate::ids;
use crate::item::Kind;
use crate::store::Store;
use args::{Cli, Command, GlobalArgs, TOP_HELP};
use resolve::Resolved;

pub fn try_parse<T: Parser>(argv: impl IntoIterator<Item = OsString>) -> clap::error::Result<T> {
    T::try_parse_from(std::iter::once(OsString::from("doen")).chain(argv))
}

fn parse<T: Parser>(argv: Vec<OsString>) -> Result<T> {
    try_parse(argv).map_err(|e| {
        let _ = e.print();
        Error::Silent(if e.use_stderr() { 2 } else { 0 })
    })
}

/// The index of the command word: the first argument that is not a global option.
fn command_word(argv: &[OsString]) -> usize {
    let cli = Cli::command();
    let mut i = 0;
    while let Some(a) = argv.get(i).and_then(|a| a.to_str()) {
        let (name, inline_value) = a.split_once('=').map_or((a, false), |(n, _)| (n, true));
        let Some(opt) = cli.get_arguments().find(|o| {
            o.get_long()
                .is_some_and(|l| name.strip_prefix("--") == Some(l))
                || o.get_short().is_some_and(|s| name == format!("-{s}"))
        }) else {
            break;
        };
        i += if opt.get_action().takes_values() && !inline_value {
            2
        } else {
            1
        };
    }
    i.min(argv.len())
}

/// Replaces the command word with the full command name, or inserts `ls` if there is none.
pub fn normalize(mut argv: Vec<OsString>) -> Result<Vec<OsString>> {
    let i = command_word(&argv);
    let word = argv.get(i).map(|w| w.to_string_lossy().into_owned());
    match word.as_deref() {
        Some("-h" | "--help" | "-V" | "--version") => {}
        None => argv.push("ls".into()),
        Some(w) if w.starts_with('-') => argv.insert(i, "ls".into()),
        Some(w) => match resolve::resolve(w) {
            Resolved::Command(n) => argv[i] = n.into(),
            Resolved::Ambiguous(c) => {
                return Err(usage(format!("'{w}' is ambiguous: {}", c.join(", "))));
            }
            Resolved::Unknown => {
                let mut msg = format!("unknown command '{w}'");
                if is_id(&argv[..i], w) {
                    msg.push_str(&format!(". To show the item, run 'doen show {w}'"));
                }
                return Err(usage(msg));
            }
        },
    }
    Ok(argv)
}

#[derive(Parser)]
struct Globals {
    #[command(flatten)]
    global: GlobalArgs,
}

/// Whether an unknown command word is the ID of an item.
fn is_id(globals: &[OsString], word: &str) -> bool {
    if word.len() < 2 || !word.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return false;
    }
    let dir = try_parse::<Globals>(globals.to_vec())
        .ok()
        .and_then(|g| g.global.dir);
    Store::open(&store_path(dir.as_deref()), &[], false)
        .is_ok_and(|mut s| ids::resolve(&mut s, word, true).is_ok())
}

fn help(word: Option<&str>) -> Result<()> {
    let Some(w) = word else {
        print!("{TOP_HELP}");
        return Ok(());
    };
    match resolve::resolve(w) {
        Resolved::Command(n) => parse::<Cli>(vec![n.into(), "--help".into()]).map(|_| ()),
        _ => Err(usage(format!("unknown command '{w}'"))),
    }
}

pub fn run(argv: Vec<OsString>) -> Result<()> {
    let cli: Cli = parse(normalize(argv)?)?;
    let g = &cli.global;
    let app = || App::new(g.dir.as_deref(), g.color, g.yes);
    match cli.command {
        Command::Note(c) => create::run(&app()?, Kind::Note, c),
        Command::Todo(c) => create::run(&app()?, Kind::Todo, c),
        Command::Ls(c) => list::ls(&app()?, c),
        Command::Today(c) => list::today(&app()?, c),
        Command::Show(c) => show::show(&app()?, c),
        Command::Grep(c) => list::grep(&app()?, c),
        Command::Tags(c) => tags::run(&app()?, c),
        Command::Interactive => crate::ui::run(app()?),
        Command::Edit(c) => edit::run(&app()?, &c.id),
        Command::Trash(c) if !c.empty => list::trash(&app()?, c),
        Command::Init => init::run(g.dir.as_deref()),
        Command::Completion(c) => {
            completion::print(c.shell);
            Ok(())
        }
        Command::Help(c) => help(c.command.as_deref()),
        Command::Complete(c) => completion::data(&app()?, &c.what),
        cmd => {
            let app = app()?;
            app.run_tx(&[], |tx| {
                dispatch(&app, tx, cmd, &mut |tx, idx, q| app.confirm(tx, idx, q))
            })
        }
    }
}

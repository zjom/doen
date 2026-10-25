//! Command-word resolution: exact full name, then alias, then unique prefix.

use clap::CommandFactory;

use super::args::Cli;

#[derive(Debug, PartialEq)]
pub enum Resolved {
    Command(String),
    Ambiguous(Vec<String>),
    Unknown,
}

pub fn resolve(word: &str) -> Resolved {
    let cli = Cli::command();
    let commands: Vec<&clap::Command> = cli.get_subcommands().collect();
    let found = commands.iter().find(|c| c.get_name() == word).or_else(|| {
        commands
            .iter()
            .find(|c| c.get_all_aliases().any(|a| a == word))
    });
    if let Some(c) = found {
        return Resolved::Command(c.get_name().into());
    }
    // `rm` and `init` never match by prefix.
    let matches: Vec<String> = commands
        .iter()
        .filter(|c| !c.is_hide_set())
        .map(|c| c.get_name())
        .filter(|n| !["rm", "init"].contains(n) && !word.is_empty() && n.starts_with(word))
        .map(String::from)
        .collect();
    match matches.as_slice() {
        [] => Resolved::Unknown,
        [n] => Resolved::Command(n.clone()),
        _ => Resolved::Ambiguous(matches),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order() {
        let cmd = |n: &str| Resolved::Command(n.into());
        let amb = |v: &[&str]| Resolved::Ambiguous(v.iter().map(|s| s.to_string()).collect());
        assert_eq!(resolve("t"), cmd("todo"));
        assert_eq!(resolve("tag"), cmd("tag"));
        assert_eq!(resolve("arch"), cmd("archive"));
        assert_eq!(resolve("d"), cmd("done"));
        assert_eq!(resolve("r"), amb(&["reopen", "restore"]));
        assert_eq!(resolve("re"), amb(&["reopen", "restore"]));
        assert_eq!(resolve("to"), amb(&["todo", "today"]));
        assert_eq!(resolve("rm"), cmd("rm"));
        assert_eq!(resolve("i"), cmd("interactive"));
        assert_eq!(resolve("in"), cmd("interactive"));
        assert_eq!(resolve("init"), cmd("init"));
        assert_eq!(resolve("__complete"), cmd("__complete"));
        assert_eq!(resolve("_"), Resolved::Unknown);
        assert_eq!(resolve("xyz"), Resolved::Unknown);
    }
}

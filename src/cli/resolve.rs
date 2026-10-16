//! Command-word resolution: exact full name, then alias, then unique prefix.

/// Full names and aliases. `rm` never matches by prefix.
pub const COMMANDS: &[(&str, Option<&str>)] = &[
    ("note", Some("n")),
    ("todo", Some("t")),
    ("ls", None),
    ("today", None),
    ("show", Some("s")),
    ("grep", None),
    ("tags", None),
    ("edit", Some("e")),
    ("title", None),
    ("due", None),
    ("date", None),
    ("tag", None),
    ("done", Some("d")),
    ("drop", None),
    ("reopen", None),
    ("archive", None),
    ("unarchive", None),
    ("rm", None),
    ("restore", None),
    ("trash", None),
    ("init", None),
    ("check", None),
    ("completion", None),
    ("help", None),
];

#[derive(Debug, PartialEq)]
pub enum Resolved {
    Command(&'static str),
    Ambiguous(Vec<&'static str>),
    Unknown,
}

pub fn resolve(word: &str) -> Resolved {
    if word == "__complete" {
        return Resolved::Command("__complete");
    }
    if let Some((name, _)) = COMMANDS.iter().find(|(n, _)| *n == word) {
        return Resolved::Command(name);
    }
    if let Some((name, _)) = COMMANDS.iter().find(|(_, a)| *a == Some(word)) {
        return Resolved::Command(name);
    }
    let matches: Vec<&'static str> = COMMANDS
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| *n != "rm" && !word.is_empty() && n.starts_with(word))
        .collect();
    match matches.len() {
        0 => Resolved::Unknown,
        1 => Resolved::Command(matches[0]),
        _ => Resolved::Ambiguous(matches),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order() {
        assert_eq!(resolve("t"), Resolved::Command("todo"));
        assert_eq!(resolve("tag"), Resolved::Command("tag"));
        assert_eq!(resolve("arch"), Resolved::Command("archive"));
        assert_eq!(resolve("d"), Resolved::Command("done"));
        assert_eq!(resolve("r"), Resolved::Ambiguous(vec!["reopen", "restore"]));
        assert_eq!(
            resolve("re"),
            Resolved::Ambiguous(vec!["reopen", "restore"])
        );
        assert_eq!(resolve("to"), Resolved::Ambiguous(vec!["todo", "today"]));
        assert_eq!(resolve("rm"), Resolved::Command("rm"));
        assert_eq!(resolve("xyz"), Resolved::Unknown);
    }
}

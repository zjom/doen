use std::path::Path;
use std::process::Command;

use crate::app::App;
use crate::error::{Error, Result, general};
use crate::item::Item;
use crate::term;

/// Runs the editor through `sh`, so that the editor setting can contain arguments.
pub fn open(app: &App, path: &Path) -> Result<()> {
    let editor = app.cfg.editor_command();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$@\""))
        .arg("sh")
        .arg(path)
        .status()
        .map_err(|e| general(format!("cannot start the editor '{editor}': {e}")))?;
    if !status.success() {
        return Err(general(format!("the editor '{editor}' failed")));
    }
    Ok(())
}

pub fn require_terminal() -> Result<()> {
    if term::has_terminal() {
        Ok(())
    } else {
        Err(general("the editor requires a terminal"))
    }
}

/// Asks whether to open the editor again. Returns `Ok(())` to edit again.
pub fn edit_again(problem: &str) -> Result<()> {
    eprintln!("doen: {problem}");
    let answer = term::ask("(e)dit again or (d)iscard the changes? [E/d] ")
        .map_err(|_| Error::Declined("no terminal".into()))?;
    if answer.trim().eq_ignore_ascii_case("d") {
        Err(general("changes discarded"))
    } else {
        Ok(())
    }
}

/// Parses the edited text and checks the read-only fields against the original item.
pub fn validate(orig: &Item, text: &str) -> std::result::Result<Item, String> {
    let item =
        Item::parse(&orig.path, orig.place, text)?.ok_or("the frontmatter has no 'id' field")?;
    for key in ["id", "kind", "created"] {
        if item.doc.get(key) != orig.doc.get(key) {
            return Err(format!("the field '{key}' is read-only"));
        }
    }
    Ok(item)
}

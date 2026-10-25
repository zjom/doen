use std::path::Path;
use std::process::Command;

use tempfile::NamedTempFile;

use crate::app::App;
use crate::error::{Error, Result, general};
use crate::item::Item;
use crate::store::TMP_PREFIX;
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

/// In `dir`, so that relative links resolve in the editor.
pub fn temp_file(dir: &Path, name: &str, text: &str) -> Result<NamedTempFile> {
    let tmp = tempfile::Builder::new()
        .prefix(&format!("{TMP_PREFIX}{name}-"))
        .suffix(".md")
        .tempfile_in(dir)?;
    std::fs::write(tmp.path(), text)?;
    Ok(tmp)
}

/// Opens the editor until the text is a valid edit of `orig`, or the user discards it.
pub fn edit_valid(app: &App, path: &Path, orig: &Item) -> Result<(String, Item)> {
    loop {
        open(app, path)?;
        let text = std::fs::read_to_string(path)?;
        match validate(orig, &text) {
            Ok(item) => return Ok((text, item)),
            Err(e) => edit_again(&format!("invalid frontmatter: {e}"))?,
        }
    }
}

/// Parses the edited text and checks the read-only fields against the original item.
fn validate(orig: &Item, text: &str) -> std::result::Result<Item, String> {
    let item =
        Item::parse(&orig.path, orig.place, text)?.ok_or("the frontmatter has no 'id' field")?;
    for key in ["id", "kind", "created"] {
        if item.doc.get(key) != orig.doc.get(key) {
            return Err(format!("the field '{key}' is read-only"));
        }
    }
    Ok(item)
}

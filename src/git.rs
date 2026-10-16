//! Automatic Git commits (`git_autocommit`).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::warn;
use crate::store::Changes;

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The path relative to the top of the work tree. The file may not exist any more.
fn relative(top: &Path, p: &Path) -> Option<String> {
    let parent = p.parent()?.canonicalize().ok()?;
    let full = parent.join(p.file_name()?);
    Some(full.strip_prefix(top).ok()?.to_string_lossy().into_owned())
}

/// Commits the changed paths. A failure is a warning only.
pub fn commit(root: &Path, changes: &Changes) {
    if changes.paths.is_empty() {
        return;
    }
    let mut lines: Vec<String> = changes
        .quiet_lines
        .iter()
        .chain(&changes.lines)
        .map(|l| format!("doen: {}", l.splitn(3, "  ").collect::<Vec<_>>().join(" ")))
        .collect();
    if lines.is_empty() {
        lines.push("doen: update".into());
    }
    if let Err(e) = try_commit(root, &lines.join("\n"), &changes.paths) {
        warn(format!("no Git commit: {e}"));
    }
}

fn try_commit(root: &Path, message: &str, paths: &[PathBuf]) -> Result<(), String> {
    let top = git(root, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "the store is not in a Git work tree".to_string())?;
    let top = PathBuf::from(top.trim())
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut rels = vec![];
    for p in paths {
        let rel =
            relative(&top, p).ok_or_else(|| format!("{} is outside the work tree", p.display()))?;
        if p.exists() {
            git(&top, &["add", "-A", "--", &rel])?;
        } else {
            git(
                &top,
                &["rm", "-q", "--cached", "--ignore-unmatch", "--", &rel],
            )?;
        }
        rels.push(rel);
    }
    let mut args = vec!["diff", "--cached", "--name-only", "--no-renames", "--"];
    args.extend(rels.iter().map(String::as_str));
    let staged = git(&top, &args)?;
    let staged: Vec<&str> = staged.lines().filter(|l| !l.is_empty()).collect();
    if staged.is_empty() {
        return Ok(());
    }
    let mut args = vec!["commit", "-q", "--only", "-m", message, "--"];
    args.extend(staged);
    git(&top, &args)?;
    Ok(())
}

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

const NOW: &str = "2026-10-16T12:00:00+11:00";

struct Env {
    dir: TempDir,
    now: String,
}

struct Out {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Env {
    fn new() -> Env {
        let env = Env {
            dir: TempDir::new().unwrap(),
            now: NOW.into(),
        };
        env.ok(&["init"]);
        env
    }

    fn store(&self) -> PathBuf {
        self.dir.path().join("store")
    }

    fn cmd(&self) -> Command {
        let mut std = std::process::Command::cargo_bin("doen").unwrap();
        // A new session has no controlling terminal, so /dev/tty can't be opened and prompts
        // fail instead of waiting on the terminal running the tests.
        unsafe {
            std.pre_exec(|| rustix::process::setsid().map(drop).map_err(Into::into));
        }
        let mut c = Command::from_std(std);
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.dir.path().join("xdg"))
            .env("DOEN_NOTES_DIR", self.store())
            .env("DOEN_CONFIG", self.dir.path().join("config.yaml"))
            .env("DOEN_NOW", &self.now)
            .env("NO_COLOR", "1");
        c
    }

    fn run_with(&self, args: &[&str], stdin: &str) -> Out {
        let o = self.cmd().args(args).write_stdin(stdin).output().unwrap();
        Out {
            stdout: String::from_utf8(o.stdout).unwrap(),
            stderr: String::from_utf8(o.stderr).unwrap(),
            code: o.status.code().unwrap_or(-1),
        }
    }

    fn run(&self, args: &[&str]) -> Out {
        self.run_with(args, "")
    }

    fn ok(&self, args: &[&str]) -> String {
        let o = self.run(args);
        assert_eq!(o.code, 0, "doen {args:?} failed: {}", o.stderr);
        o.stdout
    }

    fn id(&self, args: &[&str]) -> String {
        self.ok(args).trim().to_string()
    }

    fn file(&self, id: &str) -> PathBuf {
        for dir in ["", ".archive", ".trash"] {
            let d = self.store().join(dir);
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.split('-').nth(1) == Some(id) {
                    return e.path();
                }
            }
        }
        panic!("no file for {id}");
    }

    fn read(&self, id: &str) -> String {
        fs::read_to_string(self.file(id)).unwrap()
    }
}

fn place_of(p: &Path) -> &str {
    p.parent().unwrap().file_name().unwrap().to_str().unwrap()
}

#[test]
fn create_with_interleaved_options() {
    let e = Env::new();
    let id = e.id(&["t", "buy", "-d", "fri", "milk", "-t", "Home"]);
    assert_eq!(id.len(), 4);
    let text = e.read(&id);
    assert_eq!(
        text,
        format!(
            "---\nid: \"{id}\"\nkind: todo\ncreated: {NOW}\ndate: 2026-10-16\ndue: 2026-10-23\ntags: [\"home\"]\n---\n\n# [ ] buy milk\n"
        )
    );
    assert!(
        e.file(&id)
            .to_string_lossy()
            .ends_with(&format!("20261016-{id}-buy-milk.md"))
    );
    let id = e.id(&["note", "--", "-x", "not", "an", "option"]);
    assert!(e.read(&id).contains("# -x not an option\n"));
    assert_eq!(e.run(&["n", "x", "-d", "fri"]).code, 2);
}

#[test]
fn content_from_stdin() {
    let e = Env::new();
    let id = e
        .run_with(
            &["n", "release", "notes", "-c", "-"],
            "line one\nline two\n",
        )
        .stdout;
    assert!(
        e.read(id.trim())
            .ends_with("# release notes\nline one\nline two\n")
    );
}

#[test]
fn default_list() {
    let e = Env::new();
    e.ok(&["t", "pay", "invoice", "-d", "3d", "-t", "admin"]);
    e.ok(&["t", "buy", "milk", "-d", "today", "-t", "home"]);
    e.ok(&["t", "call", "bank", "-d", "sun", "-S", "1d", "-t", "admin"]);
    e.ok(&["n", "standup", "-t", "work"]);
    e.ok(&["n", "release", "notes", "-w", "1d", "-t", "work"]);
    let out = e.ok(&[]);
    let lines: Vec<&str> = out.lines().collect();
    // Each date sits under its label.
    let col = |line: &str, s: &str| line.find(s).unwrap();
    assert!(lines[0].starts_with("TODOS") && lines[0].ends_with("due"));
    assert!(col(lines[0], "scheduled") < col(lines[0], "due"));
    assert!(
        lines[1].contains("[ ] pay invoice")
            && !lines[1].contains("today")
            && col(lines[1], "3d overdue") == col(lines[0], "due")
            && lines[1].ends_with("#admin")
    );
    assert!(lines[2].contains("buy milk") && lines[2].find("today") == lines[0].find("due"));
    assert!(
        lines[3].contains("call bank")
            && col(lines[3], "yesterday") == col(lines[0], "scheduled")
            && col(lines[3], "sun (+2d)") == col(lines[0], "due")
    );
    assert_eq!(lines[4], "");
    assert!(lines[5].starts_with("NOTES") && lines[5].ends_with("date"));
    assert!(col(lines[5], "date") > col(lines[0], "due"));
    assert_eq!(col(lines[6], "today"), col(lines[5], "date"));
    assert_eq!(col(lines[7], "yesterday"), col(lines[5], "date"));
}

#[test]
fn long_list() {
    let e = Env::new();
    let id = e.id(&["t", "pay", "invoice"]);
    e.ok(&["done", &id]);
    e.ok(&["reopen", &id]);
    e.ok(&["n", "standup", "-w", "1d"]);
    let out = e.ok(&["ls", "-l"]);
    let lines: Vec<&str> = out.lines().collect();
    let col = |line: &str, s: &str| line.find(s).unwrap();
    assert!(
        lines[0].starts_with("TODOS") && lines[0].ends_with("status"),
        "{out}"
    );
    assert!(!lines[0].contains("scheduled"), "{out}");
    assert_eq!(col(lines[1], "26-10-16 "), col(lines[0], "created"));
    assert_eq!(col(lines[1], "today"), col(lines[0], "date"));
    assert_eq!(
        col(lines[1], "done 10-16 → open 10-16"),
        col(lines[0], "status")
    );
    assert!(lines[3].starts_with("NOTES") && lines[3].ends_with("date"));
    assert_eq!(col(lines[3], "created"), col(lines[0], "created"));
    assert_eq!(col(lines[3], "date"), col(lines[0], "date"));
    assert_eq!(col(lines[4], "yesterday"), col(lines[0], "date"));
}

#[test]
fn long_titles_fit_the_terminal() {
    let e = Env::new();
    let title = "a todo title long enough to run past the edge of a narrow terminal";
    let id = e.id(&["t", title, "-t", "work"]);
    let out = e.cmd().arg("ls").env("COLUMNS", "50").output().unwrap();
    let out = String::from_utf8(out.stdout).unwrap();
    assert!(out.contains(&format!(
        "  {id}  [ ] a todo title long enough to ru…  #work\n"
    )));
    assert!(out.lines().all(|l| l.chars().count() <= 50));
    // Without a terminal, titles are whole.
    assert!(e.ok(&["ls"]).contains(title));
}

#[test]
fn ls_header_and_footer() {
    let e = Env::new();
    e.ok(&["t", "pay", "invoice"]);
    let tty = |args: &[&str], cfg: &str| {
        fs::write(e.store().join(".doen.yaml"), cfg).unwrap();
        let o = e
            .cmd()
            .env("DOEN_ASSUME_TTY", "1")
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success());
        String::from_utf8(o.stdout).unwrap()
    };
    let out = tty(&["ls"], "");
    assert!(out.starts_with(&format!("{}██████╗  ██████╗", " ".repeat(24))));
    assert!(out.contains("TODOS\n  "));
    assert!(out.contains("\n❯ doen · doen t <title> · "));
    assert!(out.trim_end().ends_with("· doen help"));
    assert!(tty(&[], "").contains("██████╗  ██████╗"));
    assert!(tty(&["ls", "--no-header"], "").starts_with("TODOS"));
    assert!(!tty(&["ls", "--no-footer"], "").contains("❯ doen"));
    let plain = tty(&["ls", "--no-header", "--no-footer"], "");
    assert!(plain.starts_with("TODOS") && plain.trim_end().ends_with("[ ] pay invoice"));
    assert_eq!(tty(&["ls"], "ls_header: false\nls_footer: false\n"), plain);
    assert_eq!(tty(&["ls", "--ids"], "").lines().count(), 1);
    assert!(!tty(&["today"], "").contains("██████╗"));
    // Not a terminal: no banner, no cheatsheet.
    fs::remove_file(e.store().join(".doen.yaml")).unwrap();
    assert_eq!(e.ok(&["ls"]), plain);
}

#[test]
fn status_archive_and_reopen() {
    let e = Env::new();
    let id = e.id(&["t", "tax", "return"]);
    assert_eq!(e.ok(&["done", &id]), format!("done  {id}  tax return\n"));
    let p = e.file(&id);
    assert_eq!(place_of(&p), ".archive");
    let text = e.read(&id);
    assert!(text.contains("archived: 2026-10-16\nstatus:\n  - date: 2026-10-16\n    status: done\n---\n\n# [x] tax return\n"));
    // Same status: warning, exit 0, no change.
    let o = e.run(&["done", &id]);
    assert_eq!(o.code, 0);
    assert!(o.stderr.contains("already done"));
    assert_eq!(e.read(&id), text);
    // Hidden from the default list, visible with -s done.
    assert_eq!(e.ok(&["ls", "--ids"]), "");
    assert_eq!(e.ok(&["-s", "done", "--ids"]), format!("{id}\n"));
    assert_eq!(e.ok(&["--closed", "1w", "--ids"]), format!("{id}\n"));
    assert_eq!(e.ok(&["--closed", "--ids"]), format!("{id}\n"));
    assert_eq!(
        e.ok(&["ls", "--closed", "-n", "1", "--ids"]),
        format!("{id}\n")
    );
    e.ok(&["reopen", &id[..2]]);
    assert_eq!(place_of(&e.file(&id)), "store");
    let text = e.read(&id);
    assert!(!text.contains("archived"));
    assert!(text.contains("# [ ] tax return"));
    // --on rules.
    assert_eq!(e.run(&["drop", "--on", "tomorrow", &id]).code, 2);
    assert_eq!(e.run(&["drop", "--on", "3d", &id]).code, 2);
    e.ok(&["drop", "--on", "today", &id]);
    assert!(e.read(&id).contains("# [-] tax return"));
}

#[test]
fn archive_closed_false() {
    let e = Env::new();
    fs::write(e.store().join(".doen.yaml"), "archive_closed: false\n").unwrap();
    let id = e.id(&["t", "x"]);
    e.ok(&["done", &id]);
    assert_eq!(place_of(&e.file(&id)), "store");
}

#[test]
fn all_or_nothing() {
    let e = Env::new();
    let a = e.id(&["t", "a"]);
    let n = e.id(&["n", "b"]);
    let o = e.run(&["done", &a, "zzzz"]);
    assert_eq!(o.code, 3);
    assert!(e.read(&a).contains("# [ ] a"));
    assert_eq!(e.run(&["done", &a, &n]).code, 2);
    assert!(e.read(&a).contains("# [ ] a"));
    assert_eq!(e.run(&["due", "tomorrow", &n]).code, 2);
}

#[test]
fn ids_from_stdin() {
    let e = Env::new();
    let a = e.id(&["t", "a", "-d", "2d"]);
    let b = e.id(&["t", "b", "-d", "3d"]);
    e.id(&["t", "c"]);
    let ids = e.ok(&["ls", "t", "--overdue", "--ids"]);
    let o = e.run_with(&["due", "tomorrow", "-"], &ids);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(e.read(&a).contains("due: 2026-10-17"));
    assert!(e.read(&b).contains("due: 2026-10-17"));
}

#[test]
fn command_resolution() {
    let e = Env::new();
    let id = e.id(&["t", "x"]);
    e.ok(&["arch", &id]);
    assert_eq!(place_of(&e.file(&id)), ".archive");
    let o = e.run(&["re", &id]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("reopen") && o.stderr.contains("restore"));
    let o = e.run(&[&id]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains(&format!("doen show {id}")));
    assert_eq!(e.run(&["bogus"]).code, 2);
}

#[test]
fn preserves_hand_written_frontmatter() {
    let e = Env::new();
    let text = "---\n# my comment\nid: k3f9\nkind: todo\ncreated: 2026-10-16T21:58:03+11:00\ndate: 2026-10-16\ntags: [Admin]\nmine: no   # keep me\nnum: 1e10\n---\n# [ ] call bank\nstuff\n";
    fs::write(e.store().join("20261016-k3f9-call-bank.md"), text).unwrap();
    e.ok(&["due", "sun", "k3f9"]);
    let out = e.read("k3f9");
    assert_eq!(
        out,
        text.replace("date: 2026-10-16\n", "date: 2026-10-16\ndue: 2026-10-18\n")
    );
    let json = e.ok(&["show", "--json", "k3f9"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v[0]["mine"], "no");
    assert_eq!(v[0]["num"], "1e10");
    assert_eq!(v[0]["current_status"], "open");
    assert_eq!(v[0]["place"], "root");
    assert_eq!(e.ok(&["ls", "-t", "admin", "--ids"]), "k3f9\n");
}

#[test]
fn ignores_non_items_and_warns_on_invalid() {
    let e = Env::new();
    fs::write(e.store().join("README.md"), "# readme\n").unwrap();
    fs::write(e.store().join("other.md"), "---\ntitle: x\n---\n# x\n").unwrap();
    fs::write(
        e.store().join("bad.md"),
        "---\nid: \"b4d0\"\nkind: what\n---\n# x\n",
    )
    .unwrap();
    let o = e.run(&["ls"]);
    assert_eq!(o.code, 0);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("bad.md"));
    assert!(!o.stderr.contains("README"));
}

#[test]
fn grep_and_filters() {
    let e = Env::new();
    let a = e.id(&[
        "n",
        "invoice",
        "notes",
        "-t",
        "work/acme",
        "-c",
        "Pay the Invoice",
    ]);
    let b = e.id(&["t", "other", "-t", "work"]);
    e.id(&["t", "home", "thing", "-t", "home"]);
    assert_eq!(e.ok(&["grep", "INVOICE", "--ids"]), format!("{a}\n"));
    assert_eq!(e.run(&["grep", "nomatch"]).code, 5);
    assert_eq!(e.ok(&["ls", "-q", "nomatch"]), "");
    assert_eq!(e.ok(&["grep", "-E", "pay.*inv", "--ids"]), format!("{a}\n"));
    let work = e.ok(&["-t", "work", "--ids"]);
    assert!(work.contains(&a) && work.contains(&b));
    assert_eq!(
        e.ok(&["-t", "work", "-x", "work/acme", "--ids"]),
        format!("{b}\n")
    );
    assert_eq!(e.ok(&["ls", "n", "--ids"]), format!("{a}\n"));
    assert_eq!(e.run(&["ls", "n", "-s", "done"]).code, 2);
    assert_eq!(
        e.ok(&["ls", "-d", "none", "-t", "work", "--ids"]),
        format!("{b}\n")
    );
    assert_eq!(
        e.ok(&["ls", "-w", "M", "-n", "1", "--ids"]).lines().count(),
        1
    );
    assert_eq!(e.run(&["ls", "-w", "10-16..10-01"]).code, 2);
    let tags = e.ok(&["tags", "--porcelain"]);
    assert_eq!(tags, "home\t1\nwork\t1\nwork/acme\t1\n");
}

#[test]
fn porcelain_format() {
    let e = Env::new();
    let id = e.id(&["t", "call", "bank", "-d", "26-10-18", "-t", "admin,x"]);
    assert_eq!(
        e.ok(&["ls", "--porcelain"]),
        format!("{id}\ttodo\topen\troot\t2026-10-16\t2026-10-18\tcall bank\tadmin,x\t\t\n")
    );
}

#[test]
fn priorities() {
    let e = Env::new();
    let low = e.id(&["t", "low", "-p", "1", "-d", "26-10-18"]);
    let high = e.id(&["t", "high", "-p", "3", "-d", "26-10-20"]);
    let none = e.id(&["t", "none"]);
    let note = e.id(&["n", "a note"]);
    let text = e.read(&high);
    assert!(
        text.contains("due: 2026-10-20\npriority: 3\ntags:"),
        "{text}"
    );
    assert_eq!(e.run(&["t", "x", "-p", "4"]).code, 2);
    assert_eq!(e.run(&["n", "x", "-p", "1"]).code, 2);

    let ids = |args: &[&str]| {
        e.ok(args)
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(
        ids(&["ls", "t", "--sort", "priority", "--ids"]),
        [&high, &low, &none].map(String::as_str).join(" ")
    );
    assert_eq!(
        ids(&["ls", "--sort", "priority", "-r", "--ids"]),
        [&low, &high, &none, &note].map(String::as_str).join(" ")
    );
    assert_eq!(
        ids(&["ls", "-p", "3,none", "--ids"]),
        [&high, &none].map(String::as_str).join(" ")
    );
    // Equal due dates: the highest priority comes first.
    let mid = e.id(&["t", "mid", "-p", "2", "-d", "26-10-18"]);
    assert_eq!(
        ids(&["ls", "t", "-p", "1,2", "--ids"]),
        [&mid, &low].map(String::as_str).join(" ")
    );
    e.ok(&["rm", "-f", "-y", &mid]);
    assert_eq!(e.run(&["ls", "n", "-p", "1"]).code, 2);
    assert_eq!(e.run(&["ls", "-p", "0"]).code, 2);

    let out = e.ok(&["ls", "t"]);
    assert!(out.contains("[ ] !   low"), "{out}");
    assert!(out.contains("[ ] !!! high"), "{out}");
    assert!(out.contains("[ ]     none"), "{out}");
    assert!(e.ok(&["ls", "--porcelain", "-p", "1"]).ends_with("\t1\t\n"));
    assert!(e.ok(&["show", &high]).contains("· open · priority 3 ·"));

    e.ok(&["priority", "2", &low, &none]);
    assert!(e.read(&none).contains("priority: 2\n"));
    let o = e.run(&["priority", "2", &low]);
    assert!(o.code == 0 && o.stderr.contains("already"), "{}", o.stderr);
    e.ok(&["priority", "none", &low]);
    assert!(!e.read(&low).contains("priority"));
    assert_eq!(e.run(&["priority", "2", &note]).code, 2);
    assert_eq!(e.run(&["priority", "5", &high]).code, 2);
}

#[test]
fn scheduled() {
    let e = Env::new();
    let later = e.id(&["t", "later", "-S", "+2d", "-d", "+5d"]);
    let started = e.id(&["t", "started", "-S", "1d"]);
    let due = e.id(&["t", "due", "-S", "2d", "-d", "today"]);
    let plain = e.id(&["t", "plain"]);
    let note = e.id(&["n", "a note"]);
    let text = e.read(&later);
    assert!(
        text.contains("date: 2026-10-16\nscheduled: 2026-10-18\ndue: 2026-10-21\ntags:"),
        "{text}"
    );
    assert_eq!(e.run(&["n", "x", "-S", "today"]).code, 2);

    let ids = |args: &[&str]| {
        e.ok(args)
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ")
    };
    // Never hidden from ls, also before its day.
    assert!(ids(&["ls", "t", "--ids"]).contains(&later));
    assert_eq!(ids(&["ls", "-S", "+1d..", "--ids"]), later);
    assert_eq!(ids(&["ls", "-S", "none", "--ids"]), plain);
    assert!(ids(&["grep", "later", "--ids"]).contains(&later));
    assert_eq!(
        ids(&["ls", "t", "-S", "W", "--sort", "scheduled", "-r", "--ids"]),
        [&later, &started, &due].map(String::as_str).join(" ")
    );
    assert_eq!(e.run(&["ls", "n", "-S", "W"]).code, 2);

    let out = e.ok(&["today"]);
    assert!(
        out.contains("DUE TODAY") && out.contains("SCHEDULED"),
        "{out}"
    );
    let scheduled = out.split("SCHEDULED").nth(1).unwrap();
    assert!(
        scheduled.contains("started") && !scheduled.contains("[ ] due"),
        "{out}"
    );
    assert!(!out.contains("later"), "{out}");

    assert!(
        e.ok(&["show", &later])
            .contains("· open · scheduled 26-10-18 · due 26-10-21")
    );
    assert!(
        e.ok(&["ls", "--porcelain", "-S", "+1d.."])
            .ends_with("\t2026-10-18\n")
    );

    // The day comes: the todo shows up.
    let mut e = e;
    e.now = "2026-10-18T09:00:00+11:00".into();
    assert!(e.ok(&["ls", "t", "--ids"]).contains(&later));
    assert!(e.ok(&["today"]).contains("later"));

    e.ok(&["schedule", "+3d", &later, &plain]);
    assert!(e.read(&plain).contains("scheduled: 2026-10-21\n"));
    let o = e.run(&["schedule", "+3d", &plain]);
    assert!(o.code == 0 && o.stderr.contains("already"), "{}", o.stderr);
    e.ok(&["schedule", "none", &plain]);
    assert!(!e.read(&plain).contains("scheduled"));
    let o = e.run(&["schedule", "today", &note]);
    assert!(
        o.code == 2 && o.stderr.contains("scheduled date"),
        "{}",
        o.stderr
    );
}

#[test]
fn show_formats() {
    let e = Env::new();
    let id = e.id(&[
        "t",
        "call",
        "bank",
        "-d",
        "26-10-18",
        "-t",
        "admin",
        "-c",
        "body text",
    ]);
    assert_eq!(
        e.ok(&["show", &id]),
        format!("call bank\n{id} · todo · open · due 26-10-18 · #admin\n\nbody text\n")
    );
    assert_eq!(e.ok(&["show", "--raw", &id]), e.read(&id));
    assert_eq!(
        e.ok(&["show", "--path", &id]).trim(),
        e.file(&id).to_string_lossy()
    );
}

#[test]
fn trash_flow_and_purge() {
    let e = Env::new();
    let id = e.id(&["n", "old", "idea"]);
    assert_eq!(e.run(&["rm", &id]).code, 4);
    e.ok(&["rm", "-y", &id]);
    assert_eq!(place_of(&e.file(&id)), ".trash");
    assert_eq!(e.run(&["done", &id]).code, 3);
    assert_eq!(e.ok(&["trash", "--ids"]), format!("{id}\n"));
    e.ok(&["restore", &id]);
    assert_eq!(place_of(&e.file(&id)), "store");
    e.ok(&["rm", "-y", &id]);
    // A read-only command 31 days later deletes nothing; a mutating command does.
    let e2 = Env {
        dir: e.dir,
        now: "2026-11-16T12:00:00+11:00".into(),
    };
    e2.ok(&["ls", "--trashed"]);
    assert!(e2.store().join(".trash").read_dir().unwrap().count() == 1);
    let o = e2.run(&["t", "new"]);
    assert!(o.stderr.contains("deleted permanently"));
    assert_eq!(e2.store().join(".trash").read_dir().unwrap().count(), 0);
}

#[test]
fn trash_empty() {
    let e = Env::new();
    let id = e.id(&["n", "x"]);
    e.ok(&["rm", "-y", &id]);
    assert_eq!(e.run(&["trash", "--empty"]).code, 4);
    e.ok(&["trash", "--empty", "-y"]);
    assert_eq!(e.store().join(".trash").read_dir().unwrap().count(), 0);
}

#[test]
fn rm_force() {
    let e = Env::new();
    let a = e.id(&["n", "a"]);
    let b = e.id(&["n", "b"]);
    e.ok(&["rm", "-y", &b]);
    assert_eq!(e.run(&["rm", "-f", &a]).code, 4);
    let o = e.ok(&["rm", "--force", "-y", &a, &b]);
    assert!(o.contains("deleted"));
    assert!(!e.store().join(".trash").read_dir().unwrap().any(|_| true));
    assert_eq!(e.ok(&["ls", "-a", "--ids"]), "");
}

#[test]
fn templates() {
    let e = Env::new();
    fs::write(
        e.store().join(".templates/meeting.md"),
        "---\ntags: [\"work\", \"meeting\"]\nproject: doen\n---\n# {{title}} ({{weekday}})\n\n## notes\n",
    )
    .unwrap();
    fs::write(
        e.store().join(".templates/todo.md"),
        "---\nscheduled: \"+1d\"\ndue: \"+2d\"\npriority: 2\n---\n## steps\n",
    )
    .unwrap();
    fs::write(e.store().join(".doen.yaml"), "default_tags: [inbox]\n").unwrap();
    let id = e.id(&["n", "standup", "-T", "meeting", "-t", "x"]);
    let text = e.read(&id);
    assert!(text.contains("tags: [\"inbox\", \"work\", \"meeting\", \"x\"]\nproject: doen\n---\n\n# standup (fri)\n\n## notes\n"), "{text}");
    let id = e.id(&["t", "plan"]);
    let text = e.read(&id);
    assert!(
        text.contains("scheduled: 2026-10-17\ndue: 2026-10-18\npriority: 2\n")
            && text.ends_with("# [ ] plan\n## steps\n"),
        "{text}"
    );
    let id = e.id(&["t", "plan", "-p", "3"]);
    assert!(e.read(&id).contains("priority: 3\n"));
    let id = e.id(&["t", "plan", "-T", "none"]);
    assert!(!e.read(&id).contains("due"));
    assert_eq!(e.run(&["n", "x", "-T", "todo"]).code, 2);
    assert_eq!(e.run(&["n", "x", "-T", "missing"]).code, 2);
}

#[test]
fn templates_dir() {
    let e = Env::new();
    let global = e.dir.path().join("xdg/doen/templates");
    fs::create_dir_all(&global).unwrap();
    fs::write(global.join("todo.md"), "## xdg\n").unwrap();
    let custom = e.dir.path().join("tpl");
    fs::create_dir_all(&custom).unwrap();
    fs::write(custom.join("todo.md"), "## custom\n").unwrap();
    let id = e.id(&["t", "a"]);
    assert!(e.read(&id).ends_with("## xdg\n"));
    fs::write(e.dir.path().join("config.yaml"), "templates_dir: ~/tpl\n").unwrap();
    let id = e.id(&["t", "b"]);
    assert!(e.read(&id).ends_with("## custom\n"));
    assert_eq!(e.ok(&["__complete", "templates"]), "todo\n");
    fs::write(e.store().join(".templates/todo.md"), "## store\n").unwrap();
    let id = e.id(&["t", "c"]);
    assert!(e.read(&id).ends_with("## store\n"));
}

fn editor(e: &Env, script: &str) -> String {
    let p = e.dir.path().join("editor.sh");
    fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
    let mut perms = fs::metadata(&p).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&p, perms).unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn edit_checkbox_applies_status() {
    let e = Env::new();
    let id = e.id(&["t", "call", "bank"]);
    let ed = editor(&e, "sed -i 's/^# \\[ \\]/# [x]/' \"$1\"");
    let o = e
        .cmd()
        .env("EDITOR", &ed)
        .env("DOEN_ASSUME_TTY", "1")
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = e.read(&id);
    assert!(
        text.contains("status:\n  - date: 2026-10-16\n    status: done\n"),
        "{text}"
    );
    assert_eq!(place_of(&e.file(&id)), ".archive");
}

#[test]
fn edit_log_wins_and_readonly_fields() {
    let e = Env::new();
    let id = e.id(&["t", "call", "bank"]);
    let ed = editor(
        &e,
        "sed -i 's/^tags: \\[\\]/tags: []\\nstatus:\\n  - date: 2026-10-16\\n    status: drop/; s/^# \\[ \\]/# [x]/' \"$1\"",
    );
    let o = e
        .cmd()
        .env("EDITOR", &ed)
        .env("DOEN_ASSUME_TTY", "1")
        .args(["edit", &id])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("disagreed"));
    assert!(e.read(&id).contains("# [-] call bank"));
    // Changing a read-only field asks to edit again; no terminal means no answer, no change.
    let id2 = e.id(&["n", "x"]);
    let before = e.read(&id2);
    let ed = editor(&e, "sed -i 's/^kind: note/kind: todo/' \"$1\"");
    let o = e
        .cmd()
        .env("EDITOR", &ed)
        .env("DOEN_ASSUME_TTY", "1")
        .args(["edit", &id2])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert_eq!(e.read(&id2), before);
}

#[test]
fn check_and_fix() {
    let e = Env::new();
    let a = e.id(&["t", "a"]);
    e.ok(&["done", &a]);
    // Move the file back to the root by hand, and break the checkbox.
    let p = e.file(&a);
    let root_path = e.store().join(p.file_name().unwrap());
    fs::write(
        &root_path,
        fs::read_to_string(&p).unwrap().replace("# [x]", "# [ ]"),
    )
    .unwrap();
    fs::remove_file(&p).unwrap();
    // A duplicate ID, created later.
    let b = e.id(&["n", "b"]);
    let dup = e.read(&b).replace(NOW, "2026-10-16T13:00:00+11:00");
    fs::write(e.store().join(format!("20261016-{b}-copy.md")), dup).unwrap();
    let o = e.run(&["check"]);
    assert_eq!(o.code, 1);
    assert!(
        o.stdout.contains("wrong directory")
            && o.stdout.contains("checkbox")
            && o.stdout.contains("duplicate id"),
        "{}",
        o.stdout
    );
    let o = e.run(&["check", "--fix"]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(e.run(&["check"]).code, 0);
    assert_eq!(place_of(&e.file(&a)), ".archive");
    assert!(e.read(&a).contains("# [x] a"));
    assert!(!e.read(&b).contains("13:00:00"));
    assert_eq!(e.ok(&["ls", "n", "--ids"]).lines().count(), 2);
}

#[test]
fn git_autocommit() {
    let e = Env::new();
    let git = |args: &[&str]| {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(e.store())
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).to_string()
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);
    fs::write(e.store().join(".doen.yaml"), "git_autocommit: true\n").unwrap();
    fs::write(e.store().join("unrelated.txt"), "x").unwrap();
    git(&["add", "unrelated.txt"]);
    let id = e.id(&["t", "call", "bank"]);
    e.ok(&["done", &id]);
    let log = git(&["log", "--format=%s"]);
    assert_eq!(
        log,
        format!("doen: done {id} call bank\ndoen: todo {id} call bank\n")
    );
    // The staged unrelated file is not committed.
    assert!(git(&["status", "--porcelain"]).contains("A  unrelated.txt"));
    assert!(git(&["ls-files"]).contains(&format!(".archive/20261016-{id}-call-bank.md")));
}

/// Writes an item file by hand: `dir` is "", ".archive" or ".trash", `extra` more frontmatter.
fn write_item(e: &Env, dir: &str, name_id: &str, id: &str, kind: &str, extra: &str, title: &str) {
    let d = e.store().join(dir);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join(format!("20261016-{name_id}-x.md")),
        format!(
            "---\nid: \"{id}\"\nkind: {kind}\ncreated: {NOW}\ndate: 2026-10-16\n{extra}---\n\n# {title}\n"
        ),
    )
    .unwrap();
}

#[test]
fn loads_only_the_places_needed() {
    let e = Env::new();
    let done = "archived: 2026-10-16\nstatus:\n  - date: 2026-10-16\n    status: done\n";
    write_item(
        &e,
        "",
        "ra01",
        "ra01",
        "todo",
        "tags: [root]\n",
        "[ ] root todo",
    );
    write_item(
        &e,
        ".archive",
        "aa01",
        "aa01",
        "todo",
        &format!("tags: [arch]\n{done}"),
        "[x] old",
    );
    write_item(
        &e,
        ".trash",
        "tt01",
        "tt01",
        "note",
        "tags: [\"bad tag\"]\ntrashed: 2026-10-15\n",
        "gone",
    );
    // IDs that share a prefix across places.
    write_item(&e, "", "zz01", "zz01", "note", "", "zz root");
    write_item(
        &e,
        ".archive",
        "zz02",
        "zz02",
        "todo",
        done,
        "[x] zz archived",
    );
    write_item(
        &e,
        ".trash",
        "yy01",
        "yy01",
        "note",
        "trashed: 2026-10-15\n",
        "yy trashed",
    );
    write_item(&e, "", "yy02", "yy02", "todo", "", "[ ] yy root");
    // A filename that does not match the ID.
    write_item(&e, "", "mm00", "mm01", "note", "", "mismatched");

    let ids = |args: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = e.ok(args).lines().map(String::from).collect();
        v.sort();
        v
    };
    assert_eq!(ids(&["ls", "--ids"]), ["mm01", "ra01", "yy02", "zz01"]);
    assert_eq!(
        ids(&["ls", "-a", "--ids"]),
        ["aa01", "mm01", "ra01", "yy02", "zz01", "zz02"]
    );
    assert_eq!(ids(&["ls", "-s", "done", "--ids"]), ["aa01", "zz02"]);
    assert_eq!(ids(&["ls", "--archived", "--ids"]), ["aa01", "zz02"]);
    assert_eq!(ids(&["ls", "--trashed", "--ids"]), ["tt01", "yy01"]);
    let tags = e.ok(&["tags"]);
    assert!(tags.contains("root") && !tags.contains("arch"), "{tags}");
    let tags = e.ok(&["tags", "--all"]);
    assert!(tags.contains("arch") && !tags.contains("bad"), "{tags}");

    // `check` sees every place.
    let o = e.run(&["check"]);
    assert!(
        o.stdout.contains("mm00") && o.stdout.contains("invalid tag"),
        "{}",
        o.stdout
    );

    assert!(e.ok(&["show", "aa01"]).contains("old"));
    assert!(e.ok(&["show", "tt01"]).contains("gone"));
    assert!(e.ok(&["show", "mm01"]).contains("mismatched"));
    assert_eq!(e.run(&["done", "tt01"]).code, 3);
    let o = e.run(&["show", "zz"]);
    assert!(
        o.code == 3 && o.stderr.contains("ambiguous"),
        "{}",
        o.stderr
    );
    assert!(e.run(&["show", "yy"]).stderr.contains("ambiguous"));
    // Without the trash, the prefix is unique.
    e.ok(&["done", "yy"]);
    assert_eq!(place_of(&e.file("yy02")), ".archive");
    e.ok(&["unarchive", "aa01"]);
    assert_eq!(place_of(&e.file("aa01")), "store");
    e.ok(&["restore", "tt01"]);
    assert_eq!(place_of(&e.file("tt01")), "store");
    // New IDs avoid unparsed files in every place.
    assert_eq!(e.run(&["note", "new"]).code, 0);
}

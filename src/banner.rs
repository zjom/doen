//! The header and the footer of `doen ls`.

use crate::commands::App;

/// The banner text.
const BANNER: &str = "do it";

/// Glyphs in the "ANSI Shadow" style, six rows each.
fn glyph(c: char) -> [&'static str; 6] {
    match c {
        'd' => [
            "██████╗ ",
            "██╔══██╗",
            "██║  ██║",
            "██║  ██║",
            "██████╔╝",
            "╚═════╝ ",
        ],
        'o' => [
            " ██████╗ ",
            "██╔═══██╗",
            "██║   ██║",
            "██║   ██║",
            "╚██████╔╝",
            " ╚═════╝ ",
        ],
        'i' => ["██╗", "██║", "██║", "██║", "██║", "╚═╝"],
        't' => [
            "████████╗",
            "╚══██╔══╝",
            "   ██║   ",
            "   ██║   ",
            "   ██║   ",
            "   ╚═╝   ",
        ],
        _ => ["   "; 6],
    }
}

/// Renders the text in big letters, one string per row.
fn big(text: &str) -> Vec<String> {
    (0..6)
        .map(|row| {
            let line: Vec<&str> = text.chars().map(|c| glyph(c)[row]).collect();
            line.join("").trim_end().to_string()
        })
        .collect()
}

/// Colors the blocks and dims the shadow.
fn paint_row(app: &App, row: &str) -> String {
    if !app.color {
        return row.to_string();
    }
    let mut out = String::new();
    let mut run = String::new();
    let mut solid = None;
    for c in row.chars() {
        let s = c == '█';
        if solid != Some(s) && !run.is_empty() {
            out.push_str(&app.paint(if solid == Some(true) { "1;36" } else { "2" }, &run));
            run.clear();
        }
        solid = Some(s);
        run.push(c);
    }
    out.push_str(&app.paint(if solid == Some(true) { "1;36" } else { "2" }, &run));
    out
}

/// The width of the terminal: `$COLUMNS`, then the size of the terminal on stdout, then 80.
fn terminal_width() -> usize {
    if let Some(n) = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0)
    {
        return n;
    }
    #[cfg(unix)]
    if let Ok(size) = rustix::termios::tcgetwinsize(std::io::stdout())
        && size.ws_col > 0
    {
        return size.ws_col.into();
    }
    80
}

/// Prints a horizontal rule across the terminal.
fn print_rule(app: &App, width: usize) {
    println!("{}", app.paint("2", &"─".repeat(width)));
}

pub fn print_header(app: &App) {
    let width = terminal_width();
    let rows = big(BANNER);
    let banner_w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let indent = " ".repeat(width.saturating_sub(banner_w) / 2);
    for row in rows {
        println!("{indent}{}", paint_row(app, &row));
    }
    print_rule(app, width);
    println!();
}

const CHEATSHEET: [&str; 13] = [
    "doen",
    "doen t <title>",
    "doen n <title>",
    "doen e <id>",
    "doen s <id>",
    "doen done <id>",
    "doen drop <id>",
    "doen due <date> <id>",
    "doen today",
    "doen ls -t <tag>",
    "doen grep <text>",
    "doen rm <id>",
    "doen help",
];

/// Prints the cheatsheet after a prompt icon, the commands separated by `·`. A line breaks only
/// between two commands, and the next line lines up with the first command.
pub fn print_footer(app: &App) {
    let width = terminal_width();
    println!();
    print_rule(app, width);
    // The line holds painted text, so `used` counts the visible width.
    let dot = app.paint("36", "·");
    let mut lines: Vec<String> = vec![];
    let mut line = String::new();
    let mut used = 2;
    for (n, cmd) in CHEATSHEET.iter().enumerate() {
        let sep = if n == 0 { 0 } else { 3 };
        // Room for the ` ·` that ends the line if the next command breaks.
        let tail = if n + 1 == CHEATSHEET.len() { 0 } else { 2 };
        let w = cmd.chars().count();
        if n > 0 && used + sep + w + tail > width {
            lines.push(format!("{} {dot}", std::mem::take(&mut line)));
            line.push_str(&app.paint("2", cmd));
            used = 2 + w;
            continue;
        }
        if n > 0 {
            line.push_str(&format!(" {dot} "));
        }
        line.push_str(&app.paint("2", cmd));
        used += sep + w;
    }
    lines.push(line);
    for (n, l) in lines.iter().enumerate() {
        let prompt = if n == 0 { "❯ " } else { "  " };
        println!("{}{l}", app.paint("2", prompt));
    }
}

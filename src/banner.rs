//! The header and the footer of `doen ls`.

use crate::app::App;
use crate::term;

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
    let mut rest = row;
    while let Some(first) = rest.chars().next() {
        let solid = first == '█';
        let end = rest.find(|c| (c == '█') != solid).unwrap_or(rest.len());
        out.push_str(&app.paint(if solid { "1;36" } else { "2" }, &rest[..end]));
        rest = &rest[end..];
    }
    out
}

fn print_rule(app: &App, width: usize) {
    println!("{}", app.paint("2", &"─".repeat(width)));
}

pub fn print_header(app: &App) {
    let width = term::width().unwrap_or(80);
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
    let width = term::width().unwrap_or(80);
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

mod app;
mod banner;
mod cli;
mod commands;
mod complete;
mod config;
mod dates;
mod editor;
mod error;
mod filter;
mod frontmatter;
mod git;
mod ids;
mod item;
mod output;
mod store;
mod tags;
mod template;
mod term;
#[cfg(test)]
mod testing;
mod ui;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Err(e) = cli::run(args) {
        let msg = e.to_string();
        if !msg.is_empty() {
            eprintln!("doen: {msg}");
        }
        std::process::exit(e.exit_code());
    }
}

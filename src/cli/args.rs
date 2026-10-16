//! Argument structs for each command.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    /// Use the store at PATH instead of the store in $DOEN_NOTES_DIR
    #[arg(long, value_name = "PATH")]
    pub dir: Option<PathBuf>,
    /// When to color the output: auto, always or never
    #[arg(long, value_name = "WHEN")]
    pub color: Option<String>,
    /// Skip confirmation prompts
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct FilterArgs {
    /// Status: open (default), done, drop, closed or any. Implies todo
    #[arg(short = 's', long, value_name = "S")]
    pub status: Option<String>,
    /// Tags: a comma means OR, a repeated option means AND
    #[arg(short = 't', long, value_name = "TAGS")]
    pub tag: Vec<String>,
    /// Exclude items with one or more of these tags
    #[arg(short = 'x', long, value_name = "TAGS")]
    pub exclude_tag: Vec<String>,
    /// Filter on the date field
    #[arg(short = 'w', long, value_name = "RANGE")]
    pub date: Option<String>,
    /// Filter on the day of the created timestamp
    #[arg(long, value_name = "RANGE")]
    pub created: Option<String>,
    /// Filter on the due date (`none` for no due date). Implies todo
    #[arg(short = 'd', long, value_name = "RANGE")]
    pub due: Option<String>,
    /// Filter on the date the todo was closed. Implies todo and -s closed
    #[arg(long, value_name = "RANGE")]
    pub closed: Option<String>,
    /// Open todos with a due date before today
    #[arg(long)]
    pub overdue: bool,
    /// Case-insensitive substring match in the title and the body
    #[arg(short = 'q', long, value_name = "TEXT")]
    pub query: Option<String>,
    /// Read the -q value as a regular expression
    #[arg(short = 'E', long)]
    pub regex: bool,
    /// Include archived items
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Only archived items
    #[arg(long)]
    pub archived: bool,
    /// Only items in the trash
    #[arg(long)]
    pub trashed: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct OutputArgs {
    /// Sort by due, date, created, closed, title or status
    #[arg(long, value_name = "FIELD")]
    pub sort: Option<String>,
    /// Reverse the sort order
    #[arg(short = 'r', long)]
    pub reverse: bool,
    /// Show at most N items
    #[arg(short = 'n', long, value_name = "N")]
    pub limit: Option<usize>,
    /// Show the date, the creation time and the status history
    #[arg(short = 'l', long)]
    pub long: bool,
    #[command(flatten)]
    pub format: FormatArgs,
}

#[derive(Args, Debug, Clone, Default)]
pub struct FormatArgs {
    /// Print only the IDs
    #[arg(long, conflicts_with_all = ["porcelain", "json"])]
    pub ids: bool,
    /// Print a stable tab-separated format
    #[arg(long, conflicts_with = "json")]
    pub porcelain: bool,
    /// Print JSON
    #[arg(long)]
    pub json: bool,
}

/// Create a note or a todo
#[derive(Parser, Debug)]
pub struct CreateCmd {
    /// The title words
    pub title: Vec<String>,
    /// The body text (`-` reads stdin)
    #[arg(short = 'c', long, value_name = "TEXT", allow_hyphen_values = true)]
    pub content: Option<String>,
    /// Tags, separated by commas
    #[arg(short = 't', long, value_name = "TAGS")]
    pub tag: Vec<String>,
    /// Use a template (`none` disables the default template)
    #[arg(short = 'T', long, value_name = "NAME")]
    pub template: Option<String>,
    /// The date of the item (default: today)
    #[arg(short = 'w', long, value_name = "DATE")]
    pub date: Option<String>,
    /// The due date (todos only)
    #[arg(short = 'd', long, value_name = "DATE")]
    pub due: Option<String>,
    /// Open the item in the editor after creation
    #[arg(short = 'e', long)]
    pub edit: bool,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// List items
#[derive(Parser, Debug)]
pub struct LsCmd {
    /// note|n or todo|t
    pub kind: Option<String>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
    /// Do not print the banner above the list
    #[arg(long)]
    pub no_header: bool,
    /// Do not print the command cheatsheet below the list
    #[arg(long)]
    pub no_footer: bool,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Full-text search (exit code 5 if nothing matches)
#[derive(Parser, Debug)]
pub struct GrepCmd {
    pub pattern: String,
    /// note|n or todo|t
    pub kind: Option<String>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// List or empty the trash
#[derive(Parser, Debug)]
pub struct TrashCmd {
    /// Delete the items in the trash permanently
    #[arg(long)]
    pub empty: bool,
    /// note|n or todo|t
    pub kind: Option<String>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Agenda: overdue todos, todos due today, today's notes
#[derive(Parser, Debug)]
pub struct TodayCmd {
    /// Tags: a comma means OR, a repeated option means AND
    #[arg(short = 't', long, value_name = "TAGS")]
    pub tag: Vec<String>,
    /// Exclude items with one or more of these tags
    #[arg(short = 'x', long, value_name = "TAGS")]
    pub exclude_tag: Vec<String>,
    #[command(flatten)]
    pub output: OutputArgs,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Print items
#[derive(Parser, Debug)]
pub struct ShowCmd {
    /// Print the files exactly as stored
    #[arg(long, conflicts_with = "path")]
    pub raw: bool,
    /// Print only the file paths
    #[arg(long)]
    pub path: bool,
    #[command(flatten)]
    pub format: FormatArgs,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// List tags with counts
#[derive(Parser, Debug)]
pub struct TagsCmd {
    /// Count items in the store root and in the archive
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Print a stable tab-separated format
    #[arg(long, conflicts_with = "json")]
    pub porcelain: bool,
    /// Print JSON
    #[arg(long)]
    pub json: bool,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Open an item in the editor
#[derive(Parser, Debug)]
pub struct EditCmd {
    pub id: String,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Change the title
#[derive(Parser, Debug)]
pub struct TitleCmd {
    pub id: String,
    #[arg(required = true)]
    pub title: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Set (or with `none`, clear) a date field
#[derive(Parser, Debug)]
pub struct DateCmd {
    pub date: String,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Add or remove tags
#[derive(Parser, Debug)]
pub struct TagCmd {
    #[command(subcommand)]
    pub action: TagAction,
}

#[derive(Subcommand, Debug)]
pub enum TagAction {
    /// Add tags
    Add(TagArgs),
    /// Remove tags
    Rm(TagArgs),
}

#[derive(Args, Debug)]
pub struct TagArgs {
    /// Tags, separated by commas
    pub tags: String,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Change the status of todos
#[derive(Parser, Debug)]
pub struct StatusCmd {
    /// The date of the status change (default: today)
    #[arg(long, value_name = "DATE")]
    pub on: Option<String>,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// A command that takes only IDs
#[derive(Parser, Debug)]
pub struct IdsCmd {
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Create the store and the default config
#[derive(Parser, Debug)]
pub struct InitCmd {
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Validate the store
#[derive(Parser, Debug)]
pub struct CheckCmd {
    /// Repair what can be repaired
    #[arg(long)]
    pub fix: bool,
    #[command(flatten)]
    pub global: GlobalArgs,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

/// Print a shell completion script
#[derive(Parser, Debug)]
pub struct CompletionCmd {
    pub shell: Shell,
    #[command(flatten)]
    pub global: GlobalArgs,
}

/// Show help
#[derive(Parser, Debug)]
pub struct HelpCmd {
    pub command: Option<String>,
}

/// Completion data (hidden)
#[derive(Parser, Debug)]
pub struct CompleteCmd {
    pub what: String,
    #[command(flatten)]
    pub global: GlobalArgs,
}

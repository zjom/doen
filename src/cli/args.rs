use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::config::ColorMode;
use crate::filter::{SortField, StatusFilter};
use crate::item::Kind;
use crate::output::Format;

pub const TOP_HELP: &str = "\
doen — a notebook for the terminal

Usage: doen [command] [options] [args]

Create:
  note|n <title...>          create a note
  todo|t <title...>          create a todo

Read:
  [ls] [note|todo] [filters] list (default command)
  today                      overdue, due today, scheduled, today's notes
  show|s <id>...             print items
  grep <pattern> [filters]   full-text search
  tags [-a]                  list tags with counts
  ui                         interactive mode

Edit:
  edit|e <id>                open in the editor
  title <id> <title...>      change the title
  schedule <d|none> <id>...  set or clear the scheduled (start) date
  due <date|none> <id>...    set or clear the due date
  priority <p|none> <id>...  set or clear the priority (1-3, 3 highest)
  date <date> <id>...        set the date of the item
  tag add|rm <tags> <id>...  add or remove tags

Todo status:
  done|d [--on <date>] <id>...
  drop [--on <date>] <id>...
  reopen [--on <date>] <id>...

Lifecycle:
  archive <id>...            unarchive <id>...
  rm [-f] <id>...            move to the trash (-f: delete permanently)
  restore <id>...            restore from the trash
  trash [--empty]            list or empty the trash

Meta:
  init                       create the store and the default config
  check [--fix]              validate the store
  completion bash|zsh|fish   print a completion script
  help [command]

Global options:
  --dir <path>  --color auto|always|never  -y, --yes  -h, --help  --version

Run 'doen help <command>' for the options of a command.
";

#[derive(Parser, Debug)]
#[command(name = "doen", version, override_help = TOP_HELP, disable_help_subcommand = true)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Command,
}

/// The order of the variants is the order of the candidates in an ambiguous-prefix error.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a note
    #[command(alias = "n")]
    Note(CreateCmd),
    /// Create a todo
    #[command(alias = "t")]
    Todo(CreateCmd),
    /// List items
    Ls(LsCmd),
    /// Agenda: overdue todos, todos due today, scheduled todos, today's notes
    Today(TodayCmd),
    /// Print items
    #[command(alias = "s")]
    Show(ShowCmd),
    /// Full-text search (exit code 5 if nothing matches)
    Grep(GrepCmd),
    /// List tags with counts
    Tags(TagsCmd),
    /// Interactive mode
    Ui,
    /// Open an item in the editor
    #[command(alias = "e")]
    Edit(EditCmd),
    /// Change the title
    Title(TitleCmd),
    /// Set (or with `none`, clear) the scheduled (start) date
    Schedule(DateCmd),
    /// Set (or with `none`, clear) the due date
    Due(DateCmd),
    /// Set the date of the item
    Date(DateCmd),
    /// Set (or with `none`, clear) the priority: 1 to 3, 3 being the highest
    Priority(PriorityCmd),
    /// Add or remove tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Mark todos done
    #[command(alias = "d")]
    Done(StatusCmd),
    /// Mark todos dropped
    Drop(StatusCmd),
    /// Reopen todos
    Reopen(StatusCmd),
    /// Archive items
    Archive(IdsCmd),
    /// Unarchive items
    Unarchive(IdsCmd),
    /// Move items to the trash
    Rm(RmCmd),
    /// Restore items from the trash
    Restore(IdsCmd),
    /// List or empty the trash
    Trash(TrashCmd),
    /// Create the store and the default config
    Init,
    /// Validate the store
    Check(CheckCmd),
    /// Print a shell completion script
    Completion(CompletionCmd),
    /// Show help
    Help(HelpCmd),
    #[command(name = "__complete", hide = true)]
    Complete(CompleteCmd),
}

#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    /// Use the store at PATH instead of the store in $DOEN_NOTES_DIR
    #[arg(long, value_name = "PATH", global = true)]
    pub dir: Option<PathBuf>,
    /// When to color the output
    #[arg(long, value_name = "WHEN", global = true)]
    pub color: Option<ColorMode>,
    /// Skip confirmation prompts
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct TagFilterArgs {
    /// Tags: a comma means OR, a repeated option means AND
    #[arg(short = 't', long, value_name = "TAGS")]
    pub tag: Vec<String>,
    /// Exclude items with one or more of these tags
    #[arg(short = 'x', long, value_name = "TAGS")]
    pub exclude_tag: Vec<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct FilterArgs {
    /// Status (default: open). Implies todo
    #[arg(short = 's', long, value_name = "S")]
    pub status: Option<StatusFilter>,
    #[command(flatten)]
    pub tags: TagFilterArgs,
    /// Filter on the date field
    #[arg(short = 'w', long, value_name = "RANGE")]
    pub date: Option<String>,
    /// Filter on the day of the created timestamp
    #[arg(long, value_name = "RANGE")]
    pub created: Option<String>,
    /// Filter on the scheduled date (`none` for no scheduled date). Shows todos scheduled after
    /// today. Implies todo
    #[arg(short = 'S', long, value_name = "RANGE")]
    pub scheduled: Option<String>,
    /// Filter on the due date (`none` for no due date). Implies todo
    #[arg(short = 'd', long, value_name = "RANGE")]
    pub due: Option<String>,
    /// Priorities, separated by commas (`none` for no priority). Implies todo
    #[arg(short = 'p', long, value_name = "P")]
    pub priority: Option<String>,
    /// Closed todos, optionally only those closed in RANGE. Implies todo and -s closed
    #[arg(long, value_name = "RANGE", num_args = 0..=1)]
    pub closed: Option<Option<String>>,
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
    /// Sort field
    #[arg(long, value_name = "FIELD")]
    pub sort: Option<SortField>,
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

impl FormatArgs {
    pub fn format(&self) -> Format {
        if self.ids {
            Format::Ids
        } else if self.porcelain {
            Format::Porcelain
        } else if self.json {
            Format::Json
        } else {
            Format::Human
        }
    }
}

#[derive(Args, Debug)]
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
    /// The day to start the todo (todos only)
    #[arg(short = 'S', long, value_name = "DATE")]
    pub scheduled: Option<String>,
    /// The due date (todos only)
    #[arg(short = 'd', long, value_name = "DATE")]
    pub due: Option<String>,
    /// The priority: 1 to 3, 3 being the highest (todos only)
    #[arg(short = 'p', long, value_name = "P", value_parser = clap::value_parser!(u8).range(1..=3))]
    pub priority: Option<u8>,
    /// Open the item in the editor after creation
    #[arg(short = 'e', long)]
    pub edit: bool,
}

#[derive(Args, Debug)]
pub struct LsCmd {
    pub kind: Option<Kind>,
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
}

#[derive(Args, Debug)]
pub struct GrepCmd {
    pub pattern: String,
    pub kind: Option<Kind>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct TrashCmd {
    /// Delete the items in the trash permanently
    #[arg(long)]
    pub empty: bool,
    pub kind: Option<Kind>,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
pub struct TodayCmd {
    #[command(flatten)]
    pub tags: TagFilterArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args, Debug)]
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
}

#[derive(Args, Debug)]
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
}

#[derive(Args, Debug)]
pub struct EditCmd {
    pub id: String,
}

#[derive(Args, Debug)]
pub struct TitleCmd {
    pub id: String,
    #[arg(required = true)]
    pub title: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DateCmd {
    pub date: String,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PriorityCmd {
    /// 1, 2, 3 or `none`
    pub priority: String,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
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
}

#[derive(Args, Debug)]
pub struct StatusCmd {
    /// The date of the status change (default: today)
    #[arg(long, value_name = "DATE")]
    pub on: Option<String>,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
}

#[derive(Args, Debug)]
pub struct IdsCmd {
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RmCmd {
    /// Delete the items permanently instead of moving them to the trash
    #[arg(short, long)]
    pub force: bool,
    #[arg(required = true, value_name = "ID")]
    pub id: Vec<String>,
}

#[derive(Args, Debug)]
pub struct CheckCmd {
    /// Repair what can be repaired
    #[arg(long)]
    pub fix: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Args, Debug)]
pub struct CompletionCmd {
    pub shell: Shell,
}

#[derive(Args, Debug)]
pub struct HelpCmd {
    pub command: Option<String>,
}

#[derive(Args, Debug)]
pub struct CompleteCmd {
    pub what: String,
}

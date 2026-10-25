# doen — CLI API & Design Spec

## Overview

doen is a simple notebook for the terminal. Each note and each todo is one Markdown file with YAML frontmatter. Because of this format, you can search the files with `grep`, compare the files with `diff` and keep the files in a Git repository.

### Terms

| Term | Meaning |
| --- | --- |
| item | A note or a todo. |
| store | The directory that contains the items. |
| store root | The top level of the store. The store root does not include `.archive/` and `.trash/`. |
| archive | The `.archive/` directory of the store. |
| trash | The `.trash/` directory of the store. |
| closed todo | A todo with the status `done` or the status `drop`. |
| status log | The `status` field of a todo. The status log is a list of status changes. |
| mutating command | A command that changes one or more files in the store. |
| option | A command-line option, for example `-t` or `--json`. |
| delete permanently | Remove a file from the disk. The `doen rm` command moves items to the trash. Only `doen rm --force` deletes permanently. |

### Design principles

- **Frontmatter is the single source of truth.** The frontmatter stores the kind, the status, the archive state, the trash state and the dates of an item. doen derives the heading checkbox and the directory of the file from the frontmatter. doen rewrites the heading checkbox and moves the file to match the frontmatter. There is one exception: a manual change to the checkbox in `doen edit`. The Editing section describes this exception.
- **One meaning per option.** An option has the same meaning in every command. In a list command, an option filters the items. In a create command or a metadata command, the same option sets the same field.
- **Fast capture is the main use case.** The command `doen t buy milk -d fri` works without quotation marks and without extra command words.
- **doen does not delete data by accident.** Each status change, archive operation and trash operation has an inverse command (`reopen`, `unarchive`, `restore`). The `doen rm` command moves items to the trash. doen deletes an item in the trash permanently only after the `trash_days` period, or if you give `doen rm --force` or `doen trash --empty`. doen does not keep versions of content changes. For a full history, use the `git_autocommit` config key.
- **IDs are stable.** The ID of an item does not change when you edit the file, change the title, close the todo or archive the item. There is one exception: the repair of a duplicate ID (see IDs).
- **doen is scriptable.**
  - doen writes results to stdout.
  - doen writes warnings, prompts and errors to stderr.
  - The porcelain format is stable.
  - Read commands accept the `--json` option.
  - Each exit code has a fixed meaning.
  - Commands can read IDs from stdin.

## Command reference

Each command has one full name and a maximum of one short alias. doen resolves a command word in this order:

1. An exact full name.
2. An alias.
3. A unique prefix of a full name. For example, `doen arch` is the same as `doen archive`.

Because of this order, `doen t` always resolves to `todo`. `doen tag` always resolves to `tag`, never to `tags`.

- doen never matches the `rm` command by prefix. You must type `rm` in full.
- If a prefix matches more than one command (`doen re`, `doen to`), doen shows an error that lists the candidate commands.
- doen never matches subcommands and arguments (`tag add`, `completion bash`) by prefix.
- Prefixes are for interactive use. A command that is added in a later version can make a prefix ambiguous. We recommend that scripts use full names.

If a word is not a command, an alias or a prefix, doen shows an error and exits with code 2. doen does not show a list. If the word matches an ID, the error message suggests the command `doen show <id>`.

Commands that take IDs accept more than one ID. The IDs are always the last arguments, so that you can use the `xargs` command. The argument `-` reads IDs from stdin. Whitespace separates the IDs on stdin. You can use the `-` argument together with other ID arguments.

```
# create
doen note|n <title...>   [create-opts]       create a note
doen todo|t <title...>   [create-opts]       create a todo

# read
doen [ls] [filters]                          list (default command)
doen today                                   agenda: overdue, due today, scheduled, today's notes
doen show|s <id>...                          print to stdout
doen grep <pattern> [filters]                full-text search (ls -q with its own exit code)
doen tags [-a]                               list tags with counts
doen ui                                      interactive mode

# edit
doen edit|e <id>                             open in the editor
doen title <id> <title...>                   retitle
doen schedule <date|none> <id>...            set or clear scheduled (start) date
doen due <date|none> <id>...                 set or clear due date
doen priority <1-3|none> <id>...             set or clear priority (3 is the highest)
doen date <date> <id>...                     set the item's date
doen tag add|rm <tags> <id>...               add or remove tags

# todo status
doen done|d [--on <date>] <id>...            mark done
doen drop [--on <date>] <id>...              mark not doing
doen reopen [--on <date>] <id>...            back to open

# lifecycle
doen archive <id>...                         archive
doen unarchive <id>...                       unarchive
doen rm [-f] <id>...                         move to trash, -f deletes (asks to confirm)
doen restore <id>...                         restore from trash
doen trash [--empty] [filters]               list or empty trash

# meta
doen init                                    create store and default config
doen check [--fix]                           validate the store
doen completion bash|zsh|fish                print shell completion script
doen help [command] | --help | --version
```

The global options that follow are valid for every command:

| Option | Meaning |
| --- | --- |
| `--dir <path>` | Uses the store at `<path>` instead of the store in `$DOEN_NOTES_DIR`. |
| `--color auto\|always\|never` | Sets when doen colors the output. The default value is `auto`. For the interaction with `NO_COLOR`, see Config. |
| `-y, --yes` | Skips confirmation prompts. doen reads the answer to a prompt from the terminal, not from stdin. If no terminal is available and you do not give `-y`, doen stops without changes and exits with code 4. |
| `-h, --help` | Shows help for the command. |
| `--version` | Prints the version. |

## Creating notes and todos

The `doen note` command and the `doen todo` command always create an item. These commands never list items. Thus, if you forget the title, the command does not become a list command.

```
doen note|n <title...> [create-opts]
doen todo|t <title...> [create-opts]
```

- **Title:**
  - doen joins all positional words with spaces to make the title.
  - For plain words, quotation marks are optional. The shell still interprets the characters `'`, `"`, `#`, `*`, `?`, `&`, `;`, `(`, `)` and `!`. If a title contains one of these characters, enclose the title in quotation marks.
  - Options can appear at any position among the words. For example, the title of `doen t buy -d fri milk` is "buy milk".
  - doen reads every word that starts with `-` as an option.
  - After the `--` argument, doen reads every word as a title word. Put options before `--`.
- **No title:** If you give no title, doen opens the editor on a new file. If a template applies, the new file starts from the template. This mode requires a terminal. If you save the file with an empty H1 heading, doen stops and does not create an item.
- **Output:** The command prints only the ID of the new item to stdout. Thus, `id=$(doen t call bank)` works in a script.

| Option | Meaning | Applies to |
| --- | --- | --- |
| `-c, --content <text>` | Sets the body text. The value `-` reads the body from stdin. doen reads stdin only if you give `-c -`. Thus, doen is safe in `while read` loops and in cron jobs. | note, todo |
| `-t, --tag <tags>` | Sets the tags. Separate the tags with commas. You can give this option more than one time. | note, todo |
| `-T, --template <name>` | Uses the template `<name>` (see Templates). The value `none` disables the default template. | note, todo |
| `-w, --date <date>` | Sets the date of the item (see Frontmatter fields). The default value is today. | note, todo |
| `-S, --scheduled <date>` | Sets the scheduled date: the day to start the todo. | todo only (error for a note) |
| `-d, --due <date>` | Sets the due date. | todo only (error for a note) |
| `-p, --priority <p>` | Sets the priority: `1`, `2` or `3`. `3` is the highest. | todo only (error for a note) |
| `-e, --edit` | Opens the item in the editor after doen creates the item. | note, todo |

Examples:

```
doen t buy milk -d fri -t home
doen n standup -T meeting -e
git log --oneline -5 | doen n release notes -t work -c -
doen n retro for yesterday -w 1d
```

## Listing, filtering and search

If you run `doen` with no command, doen lists the open todos and the unarchived notes. Each filter reduces this set. If you give different filters, an item must match all of the filters (AND).

```
doen [ls] [note|n|todo|t] [filters] [output-opts]
doen grep <pattern> [filters]        # like doen ls -q <pattern>
doen today                           # agenda view
```

The optional positional kind argument replaces the old `-k` option. `doen ls t` lists todos. `doen ls n` lists notes.

### Scope

Three properties decide which items a list includes before doen applies the filters:

- **Kind:** The positional argument `note` or `todo`. If you omit the argument, the list includes both kinds. The options `-s`, `-S`, `-d`, `-p`, `--overdue` and `--closed` apply only to todos. Thus, these options imply `todo`. If you use one of these options with `ls n`, doen shows a usage error.
- **Place:** The directory that contains the file: the store root, `.archive/` or `.trash/`.
- **Status:** The `-s` option, for todos only. Notes have no status. The `-s` option does not affect notes.

| Options | Places that doen searches | Default status |
| --- | --- | --- |
| No option | Store root | `open` |
| `-s done`, `-s drop`, `-s closed` or `-s any` | Store root and archive | The value of `-s` |
| `-a` | Store root and archive | `any` |
| `--archived` | Archive only | `any` |
| `--trashed` | Trash only | `any` |

An explicit `-s` option always takes precedence over the default status in this table. For example, `doen -a -s open` lists the open todos in the store root and in the archive. doen searches the trash only if you give `--trashed`.

### Filters

| Option | Meaning |
| --- | --- |
| `-s, --status <s>` | Filters on the status: `open` (default), `done`, `drop`, `closed` (`done` and `drop`) or `any`. This option implies `todo`. See Scope. |
| `-t, --tag <tags>` | Filters on tags. A comma means OR. A repeated option means AND. For example, `-t a,b -t c` means (`a` or `b`) and `c`. |
| `-x, --exclude-tag <tags>` | Excludes the items that have one or more of these tags. You can give this option more than one time. |
| `-w, --date <range>` | Filters on the `date` field of the item. |
| `--created <range>` | Filters on the day of the `created` timestamp, in local time. |
| `-S, --scheduled <range>` | Filters on the scheduled date. The value `none` matches the todos that have no scheduled date. For example, `-S +1d..` lists the todos that are scheduled after today. This option implies `todo`. |
| `-d, --due <range>` | Filters on the due date. The value `none` matches the todos that have no due date. This option implies `todo`. |
| `-p, --priority <p>` | Filters on the priority. Separate the values with commas. The values are `1`, `2`, `3` and `none`. The value `none` matches the todos that have no priority. For example, `-p 2,3` matches the todos with priority 2 or 3. This option implies `todo`. |
| `--closed [range]` | Lists only closed todos. With a range, filters on the date of the last entry in the status log. If the next word does not start with `-`, doen reads it as the range: put `todo` before `--closed`. This option implies `todo`. If you do not give `-s`, this option also implies `-s closed`. |
| `--overdue` | Selects the open todos that have a due date before today. This option is a short form of `-s open -d ..1d`. |
| `-q, --query <text>` | Filters on a case-insensitive substring match in the title and in the body. doen does not search the frontmatter. |
| `-E, --regex` | Makes doen read the `-q` value as a case-insensitive regular expression. |
| `-a, --all` | Includes archived items. The default status becomes `any`. |
| `--archived` | Includes only archived items. The default status becomes `any`. |
| `--trashed` | Includes only the items in the trash. The default status becomes `any`. `doen trash` is the same as `doen ls --trashed`. |

### Output options

| Option | Meaning |
| --- | --- |
| `--sort <field>` | Sorts the items by a field: `due`, `scheduled`, `date`, `created`, `closed`, `title`, `status` or `priority`. The default order for todos is by due date, then by priority (highest first), then by creation time. Sorting by `due` always breaks ties by priority. The default order for notes is by date, with the newest note first. One `--sort` field applies to both groups. The `priority` field sorts the highest priority first. Notes have no `due`, `scheduled`, `closed`, `status` or `priority` field, so these five fields sort notes by date. Items that do not have the sort field come last. |
| `-r, --reverse` | Reverses the sort order. Items that do not have the sort field still come last. |
| `-n, --limit <N>` | Shows a maximum of N items in total. Todos come first. |
| `-l, --long` | Shows the creation time instead of the creation day, and adds two columns: the date and the status history. The notes use the same columns as the todos. |
| `--ids` | Prints only the IDs, one ID on each line. Use this option to send the IDs to other commands through a pipe. |
| `--porcelain` | Prints a stable output format with tab-separated columns (see Porcelain and JSON). |
| `--json` | Prints a JSON array of items (see Porcelain and JSON). |

### Header and footer

If stdout is a terminal, `doen ls` prints a banner ("do it" in big letters) above the list and a cheatsheet of common commands below the list. doen does not print the banner or the cheatsheet with `--ids`, `--porcelain` or `--json`, or if stdout is not a terminal. The other list commands (`grep`, `trash`, `today`) do not print them. doen centers the banner, draws a rule across the terminal below the banner and above the cheatsheet, and puts as many cheatsheet columns on a line as the terminal fits. The width comes from `$COLUMNS`, then from the terminal size, then defaults to 80.

| Option | Meaning |
| --- | --- |
| `--no-header` | Does not print the banner. The `ls_header` config key sets the default. |
| `--no-footer` | Does not print the cheatsheet. The `ls_footer` config key sets the default. |

### Porcelain and JSON

The `--ids` option, the `--porcelain` option and the `--json` option work with these commands: `ls`, `grep`, `today`, `trash` and `show`. The `tags` command accepts `--porcelain` (with the columns `tag` and `count`) and `--json`.

**Porcelain format:**

- The output has no header.
- Each line contains one item.
- The columns are, in this order: `id`, `kind`, `status`, `place`, `date`, `due`, `title`, `tags`, `priority`, `scheduled`.
- The `status` column is empty for notes.
- The value of the `place` column is `root`, `archive` or `trash`.
- Dates use the ISO format. If a date is not set, the column is empty.
- Commas separate the tags. The tags have no `#` character.
- doen replaces each tab and each newline in a title with a space.
- The only change that later versions make to this format is to add columns at the end.

**JSON format:**

- Each item contains every frontmatter field as stored. This content includes unknown fields and the full status log.
- Each item also contains the fields `title`, `path` (the absolute path), `place` and `current_status`. For notes, the value of `current_status` is `null`.
- Dates are ISO strings.

### Default output

Todos come first, then notes.

- For todos, two date columns show the `scheduled` field and the `due` field, in this order. If a field is not set, the cell is blank. If no item in the list has a value for a column, doen omits the column.
- For notes, the date column shows the `date` field, after the `due` column of todos.
- The header line of each group shows the labels of the date columns above the columns.
- With `-l`, the `created` column shows the creation time after the `due` column, and the columns `date` and `status` follow the `created` column. The `status` column shows the status history. The `created` column and the `date` column of notes line up with the same columns of todos.
- If a date is within `relative_days` days of today, doen shows the date as a relative date. The default value of `relative_days` is 7.
- doen highlights overdue todos.
- doen shows the priority of a todo as `!`, `!!` or `!!!` before the title. The priority column is present only if one or more items in the list have a priority.

In the example that follows, today is Friday, 2026-10-16:

```
TODOS                    scheduled  due
  w8ej  [ ] pay invoice             3d overdue  #admin
  m2xa  [ ] buy milk                today       #home
  k3f9  [ ] call bank    today      sun (+2d)   #admin

NOTES                                           date
  a7q2  standup                                 today      #work
  r4nd  release notes                           yesterday  #work
```

### The `doen grep` command

The `doen grep` command accepts the same filters and the same scope as `ls`. The only difference is the exit code. If no item matches, `grep` exits with code 5. In the same case, `ls` exits with code 0.

### The `doen today` command

The `doen today` command shows four groups:

1. Open todos that are overdue.
2. Open todos that are due today.
3. Open todos with a scheduled date of today or earlier. A todo that is in group 1 or group 2 is not in this group.
4. Notes that have today as their date.

The command does not show archived items or items in the trash. The command accepts `-t`, `-x` and the output options. For example: `doen today -t work`.

### Examples

```
doen ls t -t work -d ..+7d          # work todos due within a week
doen ls n -w M                      # notes this month
doen -s done --closed 1w            # what I finished in the last 7 days
doen grep invoice -a                # search root and archive
doen ls t --overdue --ids | xargs -r doen due tomorrow   # push all overdue to tomorrow
doen ls t -S +1d..                  # todos scheduled for later
```

## Viewing, editing and lifecycle

Every mutating command prints one line on stdout for each changed item. For example: `done  k3f9  call bank`.

Commands that take more than one ID are all-or-nothing:

1. doen first resolves and checks every ID.
2. If an ID is not found, if an ID is ambiguous, or if an ID refers to an item of the wrong kind for the command, doen changes no items.
3. doen then exits with the matching code. The code is 3 for an ID that is not found or ambiguous. The code is 2 for an item of the wrong kind.

If the new value of a field is the same as the current value, doen does not report an error. doen prints a warning, skips that item and exits with code 0.

### Viewing

- `doen show <id>...` prints these parts for each item: the title, a metadata summary on one line (`k3f9 · todo · open · due 26-10-18 · #admin`) and the body. A blank line separates the items.
- `doen show --raw` prints the files exactly as stored. If you give more than one ID, doen prints the line `==> <path> <==` before each file.
- `doen show --path` prints only the file paths, one path on each line.
- `doen tags` lists every tag with the number of items that have the tag. By default, the count includes only the items in the default scope (open todos and unarchived notes). With `-a`, the count includes the items in the store root and in the archive. Each count is for an exact tag. For example, the count for `work` does not include the items that have only the tag `work/acme`.

### Interactive mode

`doen ui` opens a full-screen interface in the style of lazygit. The command requires a terminal.

- The left panel lists the items. By default, it lists the open todos in the store root. A command (see below) replaces this list with other items. The right panel shows the selected item: the title, the metadata summary of `doen show`, the status history, the path and the body. If the terminal is less than 90 columns wide, the preview is below the list, and if it is also less than 16 rows high, doen does not show the preview. `toggle_preview` hides and shows the preview; focusing the preview shows it.
- `/` filters the list by a case-insensitive substring of the title and the body.
- `:` runs a doen command, with the same words, options and prefixes as on the command line. Quotes and backslashes work as in a shell. If the IDs are missing, the command acts on the selected item (`:due fri`, `:tag add work`, `:done --on yesterday`).
  - The list commands (`ls`, `today`, `grep`, `trash`, `show`) replace the item list with their result, as with the default command: `:--date yesterday` lists like `doen ls --date yesterday`. `--sort`, `-r` and `-n` apply; the output formats do not. The title of the list is the command. `back` returns to the open todos.
  - A line of filters lists like `ls`: a bare date or range filters by date (`:yesterday`, `:26-10-22`, `:W` is `--date`), a word that starts with `#` filters by tags (`:#life,work` is `-t life,work`), and `n` or `t` as the last word selects notes or todos. The filters compose, and options can follow: `:tomorrow #life n`, `:W #work -a`.
  - `closed [range]` exists only in the ui. It lists the closed todos, as `ls --closed [range]`, with the newest closed todo first. Filter words and options can follow: `:closed 1w #work`. `--sort` or `-r` replaces the default order.
  - `note` and `todo` create the item, as `n` and `t` do. Without a title, `n` and `t` list notes and todos. `-c -` is not available. `-e` opens the new item in the editor.
  - `edit` opens the editor and `help` shows the keybindings.
  - The other commands run doen on the same store, and the status bar shows their output. `rm` and `trash --empty` ask for confirmation first, unless you give `-y`.
  - `ui`, `init`, `tags`, `completion` and `--dir` are not available.
  - A menu above the command line shows the completions of the word that you type: commands, options, option values, IDs (with the titles; a word also matches the titles), tags and dates. `tab` puts the next completion in the line and `shift-tab` the previous one. If only one completion matches, `tab` accepts it and adds a space.
- Every key is configurable with the `ui_keys` config key (see Config). The defaults are lazygit-like: `j`/`k` move, `space` toggles done, `e` opens the editor (as `doen edit`), `d` moves to the trash, `?` lists the keys with their action names. `ctrl-c` always quits and cannot be changed.
- Text fields (prompts and the search) take every plain character as text: while you type, only the `submit` and `back` keys and the movement keys with `ctrl-`, `alt-` or a key name run an action. The editing keys of text fields (`ctrl-a`, `ctrl-e`, `ctrl-u`, `ctrl-k`, `ctrl-w`, arrows, `home`, `end`, `backspace`, `delete`) are fixed.
- In date prompts, an empty value or `none` clears the date.
- doen asks for confirmation before it moves an item to the trash or deletes it. `-y` has no effect.
- Each change is one transaction, with the same lock, file moves and Git commit as the matching command. doen reloads the store after each change; `ctrl-r` reloads it on demand.

### Editing

`doen edit <id>` copies the file to a temporary file and opens the temporary file in the editor. doen selects the editor in this order: the `editor` config key, `$VISUAL`, `$EDITOR`, `vi`. The command requires a terminal.

When you save the file, doen does these steps:

1. doen validates the frontmatter. The fields `id`, `kind` and `created` are read-only. If you change one of these fields, the frontmatter becomes invalid. If the frontmatter is invalid, doen asks you to open the editor again or to discard the changes.
2. doen checks whether the original file changed while the editor was open. For example, a `doen done` command in another shell can change the original file. If the original file changed, doen asks you to open the editor again or to discard the changes. doen never overwrites the other change.
3. doen compares the file before the edit with the file after the edit. doen uses this comparison to reconcile the heading checkbox with the status log:
   - **Only the checkbox changed:** doen applies the matching status command with today as the date: `[x]` = `done`, `[-]` = `drop`, `[ ]` = `reopen`. The status command also archives or unarchives the todo, in the same way as on the command line (see Todo status).
   - **The status log changed:** The status log takes precedence, whether or not the checkbox also changed. doen rewrites the checkbox to match the status log. If the checkbox and the status log disagreed, doen prints a warning.

   This step is the only place where a derived value changes the frontmatter. In all other places, including `doen check --fix`, doen rewrites the checkbox from the status log.
4. If the `archived` field changed, doen moves the file.
5. doen does not change the filename, even if you changed the title.

You can also change metadata without an editor:

| Command | Effect |
| --- | --- |
| `doen title <id> <title...>` | Rewrites the H1 heading. The checkbox does not change. |
| `doen schedule <date\|none> <id>...` | Sets or removes the `scheduled` field. This command applies only to todos. For a note, the command shows an error. |
| `doen due <date\|none> <id>...` | Sets or removes the `due` field. This command applies only to todos. For a note, the command shows an error. |
| `doen date <date> <id>...` | Sets the `date` field. |
| `doen tag add <tags> <id>...` | Adds the tags. Separate the tags with commas. |
| `doen tag rm <tags> <id>...` | Removes the tags. |

### Todo status

| Command | New status | Heading |
| --- | --- | --- |
| `doen done [--on <date>] <id>...` | `done` | `# [x] ...` |
| `doen drop [--on <date>] <id>...` | `drop` | `# [-] ...` |
| `doen reopen [--on <date>] <id>...` | `open` | `# [ ] ...` |

| Option | Meaning | Applies to |
| --- | --- | --- |
| `--on <date>` | Sets the date of the new entry in the status log. The default value is today. The date cannot be in the future. The date cannot be before the date of the previous entry. If the date breaks one of these rules, doen shows a usage error (exit code 2). For the date forms, see Single dates. | `done`, `drop`, `reopen` |

- Each status change adds an entry to the end of the status log. The date of the entry is today.
- Use the `--on <date>` option to give an earlier date. A date in the future is a usage error (exit code 2). Because the date cannot be before the date of the previous entry, the status log stays in date order.
- The current status is the status of the last entry. If the status log has no entries, the status is `open`.
- doen never rewrites the status log. Thus, the status log is a full history.
- If `archive_closed` is `true` (the default), `done` and `drop` also archive the todo. These commands set `archived` to the date of the entry and move the file to `.archive/`.
- `reopen` always unarchives the todo and moves the file back to the store root. This behavior does not depend on the value of `archive_closed`. This behavior also does not depend on how the todo was archived. The reason: if an open todo stays in the archive, the default list does not show the todo.
- If you change `archive_closed`, doen does not move existing files.
- If the new status is the same as the current status, the command does nothing and prints a warning. doen does not add an entry.
- A status command on a note is an error (exit code 2).

### Archive

- `doen archive <id>...` sets `archived: <today>` on any note or todo, open or closed. The command then moves the file to `.archive/`.
- The list commands do not show archived items, unless you give `-a` or `--archived`.
- `doen unarchive <id>...` removes the `archived` field and moves the file back to the store root.
- The directory follows the field. The field is the source of truth. If you set or remove `archived` by hand in `doen edit`, doen moves the file when you save the file.

### Delete and trash

- `doen rm <id>...` shows the titles and asks you to confirm. The `-y` option skips this confirmation. The command then sets `trashed: <today>` and moves the files to `.trash/`.
- `doen rm -f, --force <id>...` shows the titles and asks you to confirm. The `-y` option skips this confirmation. The command then deletes the items permanently. The IDs can refer to items in the trash.
- `doen restore <id>...` removes the `trashed` field and moves each file back. If the item has the `archived` field, doen moves the file to `.archive/`. Otherwise, doen moves the file to the store root.
- `doen trash` lists the items in the trash. The command accepts the same filters as `ls`.
- `doen trash --empty` asks you to confirm. The command then deletes the items in the trash permanently.
- If the `trashed` date of an item is more than `trash_days` days in the past, the next mutating command deletes the item permanently. The default value of the `trash_days` config key is 30. The mutating command lists the deleted items on stderr.
- Read-only commands never delete items.
- The only commands that accept an item in the trash are `show` and `restore`. Other commands report the ID as not found.

## Dates

All dates have day precision, except the `created` field. For "today" and for every comparison of days, doen uses the local time zone at the moment when the command runs. doen converts the `created` timestamp to local time before doen takes the day from the timestamp.

Relative offsets never start with `-`. Thus, doen cannot mistake an offset for an option. An offset without a sign means the past. An offset with `+` means the future.

### Single dates

| Form | Example | Meaning |
| --- | --- | --- |
| ISO | `2026-10-16` | The canonical form. doen stores dates in this form. |
| Short | `26-10-16`, `261016` | The two-digit year means 20xx. |
| Month-day | `10-16` | That day in the current year, even if the day is in the past. |
| Keyword | `today`, `yesterday`, `tomorrow` | The current day, the previous day or the next day. |
| Past offset | `0d`, `3d`, `2w`, `1m`, `1y` | Today, 3 days ago, 2 weeks ago, 1 month ago, 1 year ago. |
| Future offset | `+3d`, `+2w`, `+1m`, `+1y` | 3 days, 2 weeks, 1 month or 1 year after today. |
| Weekday | `mon` ... `sun`, `monday` ... `sunday` | The next occurrence after today. On a Friday, `fri` is 7 days after today. For the current day, use `today`. |
| Last weekday | `last-fri`, `last-friday` | The most recent occurrence before today. |
| Period end | `eow`, `eom`, `eoy` | The end of the current week, month or year. |

- Month offsets and year offsets clamp to the end of the month. For example, 31 March minus `1m` is the last day of February.
- Keywords and weekday names are case-insensitive.
- Offset units (`d`, `w`, `m`, `y`) are lowercase.
- Period letters in ranges (`W`, `M`, `Y`, `LW`, `LM`, `LY`) are uppercase.
- You can set a due date in the past, but doen prints a warning.

### Ranges (filters only)

| Form | Example | Meaning |
| --- | --- | --- |
| Single date | `26-10-16` | Only that day. |
| Closed range | `10-01..10-16` | From the start day to the end day. Both days are included. |
| Open start | `..today` | All days before today, and today. |
| Open end | `+1d..` | Tomorrow and all later days. |
| Bare past offset | `1w` | From that day to today. This form is the same as `1w..today`. |
| Bare future offset | `+7d` | From today to that day. This form is the same as `today..+7d`. |
| Period | `W`, `M`, `Y` | The whole current calendar week, month or year, including future days. |
| Last period | `LW`, `LM`, `LY` | The previous full calendar week, month or year. |
| None | `none` | The field is not set. This form is useful with `--due`. |

- To match only the day that is exactly one week ago, use `1w..1w`.
- If the start of a range is after the end of the range, doen shows a usage error.
- By default, the week starts on Monday. To change the first day of the week, set the `week_start` config key.

### Storage and display

- doen stores the `date`, `scheduled`, `due`, `archived` and `trashed` fields as ISO dates without quotation marks. For example: `due: 2026-10-18`.
- The `created` field is an ISO 8601 timestamp with a UTC offset. For example: `created: 2026-10-16T21:58:03+11:00`.
- doen reads every frontmatter scalar as a string and parses the string itself. Thus, YAML type rules never change a value, even in hand-written files. These rules include the YAML 1.1 rules and the YAML 1.2 rules for dates, `no`, `1e10` and `0012`.
- doen uses the `date_format` config key to display dates. The default value is `%y-%m-%d`.
- doen shows dates within `relative_days` days of today as relative dates (`today`, `yesterday`, `sun (+2d)`, `3d overdue`).

## Tags

A tag is a lowercase word. doen stores the tags of an item in the `tags` array. doen shows each tag with a `#` prefix.

- **Allowed characters:** `a-z`, `0-9`, `-`, `_` and `/`.
  - A tag has one or more segments. The `/` character separates the segments.
  - A segment cannot be empty. Thus, `/work`, `work/` and `a//b` are errors.
  - doen converts input to lowercase. Any other character is an error.
  - When doen reads a file, doen converts the tags in hand-written frontmatter to lowercase. `doen check` reports invalid tags in frontmatter.
- **Hierarchy:** The `/` character makes nested tags. A filter on `work` (with `-t` or with `-x`) also matches `work/acme` and all other tags below `work`. A filter on `work/acme` matches only `work/acme` and the tags below `work/acme`.
- **Editing:** `doen tag add` and `doen tag rm` operate on exact tags. For example, `doen tag rm work` does not remove `work/acme`.
- **Frontmatter only:** doen does not support inline `#tags` in the body. A `#word` in the body is plain text.
- **Typing:** Do not type the `#` character on the command line. The shell reads an unquoted word that starts with `#` as a comment.
- **Filtering:** A comma means OR. A repeated `-t` option means AND. The `-x` option excludes tags. doen has no `!` prefix for exclusion, because bash history expansion breaks the `!` prefix.
- **Default tags:** doen adds the tags in the `default_tags` config key to every new item. doen also adds the template tags and the `-t` tags. The `default_tags` key is useful in a store config file. For example, the config of a work store can always add the tag `work`.

| Filter | Matches |
| --- | --- |
| `-t home` | Items with the tag `home` |
| `-t home,admin` | Items with the tag `home`, the tag `admin`, or both tags |
| `-t home -t admin` | Items with both tags |
| `-t work -x work/acme` | Items with a `work` tag, except items with a tag in the `work/acme` branch |

## IDs

doen gives each item a random 4-character base36 ID (`0-9a-z`) when doen creates the item. doen stores the ID in the `id` frontmatter field and repeats the ID in the filename. The ID never changes, except when doen repairs a duplicate ID.

- **Generation:** doen generates a random ID and compares the ID with every ID in the store, including the archive and the trash. There are 36^4 (approximately 1.7 million) possible IDs, so collisions are rare. If the store contains more than 50,000 items, including the archive and the trash, new IDs have 5 characters. Existing IDs keep their length.
- **Matching:** Any unique prefix of 2 or more characters identifies an item (`doen done k3`). An exact full ID always takes precedence over a longer ID that starts with the same characters. For example, `k3f9` matches the ID `k3f9`, not the ID `k3f9a`. If a prefix matches more than one ID, doen shows an error that lists the matches.
- **Scope:** IDs resolve across the whole store, including closed todos and archived items. The IDs of items in the trash resolve only for `restore` and `show`. For other commands, these IDs are not found.
- **Source of truth:** The `id` frontmatter field. If a filename and its `id` field disagree (for example, after you rename the file by hand), doen uses the frontmatter and prints a warning.
- **Duplicates:** Two synchronized copies of a store can create the same ID while the copies are offline. `doen check` reports duplicate IDs. `doen check --fix` gives a new ID to the item that was created more recently. The command also renames the file of that item. This repair is the only case where an ID changes.

## Storage and file format

doen selects the store in this order: the `--dir` option, the `$DOEN_NOTES_DIR` variable, the `$HOME/doen/` directory. Archived items are in `.archive/`. Items in the trash are in `.trash/`. doen moves a file only when the file goes into or out of the archive or the trash.

doen reads only `*.md` files in the store root, in `.archive/` and in `.trash/`. doen ignores other directories, including `.templates/` and the directories of editors and of Obsidian. If the frontmatter of a Markdown file has no `id` field, the file is not a doen item. doen ignores this file and shows no message. Thus, README files and other notes can be in the store.

```
$DOEN_NOTES_DIR/
  20261016-k3f9-call-bank.md
  20261016-a7q2-standup.md
  .archive/
    20261001-p81c-tax-return.md
    20260912-c0d1-old-meeting-notes.md
  .templates/
    meeting.md
  .trash/
    20261002-z9pk-old-idea.md
  .doen.yaml                 # optional store-level config
```

- **Filename:** The format of the filename is `<created YYYYMMDD>-<id>-<slug>.md`. doen makes the slug from the title when doen creates the item:
  1. doen transliterates the title to ASCII.
  2. doen converts the title to lowercase.
  3. doen replaces each sequence of other characters with one `-` character.
  4. doen cuts the slug at a word boundary to a maximum of 40 characters.

  If the slug is empty, the filename is `<created YYYYMMDD>-<id>.md`. A title change does not rename the file. Thus, editor buffers and external references stay valid.
- **Directories:** The `.trash/` directory follows the `trashed` field. The `.archive/` directory follows the `archived` field. If both fields are set, the `trashed` field takes precedence. The frontmatter is the source of truth. When doen writes a file, doen moves the file to match the frontmatter. If a file is in the wrong directory (for example, after a manual edit outside doen), `doen check` lists the file and `doen check --fix` moves the file.
- **Body:** After the frontmatter, the first line that is not blank is the title, as an ATX H1 heading (`# title`). Free Markdown follows the title. If a file has no H1 heading, doen lists the file as `(untitled)`, and `doen check` reports the file.
- **Heading checkbox:** A todo heading has one of these forms: `# [ ] title`, `# [x] title` or `# [-] title`. doen reads `[X]` as `[x]`. doen derives the checkbox from the status log. doen rewrites the checkbox each time the status changes. Notes have no checkbox. For example, the title of a note with the heading `# [x] marks the spot` is `[x] marks the spot`.
- **Field order:** doen writes the fields in the order of the table in Frontmatter fields. doen keeps unknown fields, comments, and the format of the fields that doen does not change. Thus, you can add your own fields.
- **Quoting:** doen always puts the `id` field and the tags in quotation marks. Thus, other YAML tools do not read an ID such as `1e10` as a number. Other YAML tools also do not read a tag such as `no` or `on` as a boolean. doen itself reads every scalar as a string (see Dates). Thus, unquoted values in hand-written files are safe for doen.

### Frontmatter fields

| Field | Type | Set by | Notes |
| --- | --- | --- | --- |
| `id` | quoted string | doen, at creation | Never changes, except for the repair of a duplicate. Read-only in `doen edit`. |
| `kind` | `note` or `todo` | doen, at creation | Explicit. doen never infers the kind from the heading. Read-only in `doen edit`. |
| `created` | ISO timestamp | doen, at creation | Never changes. Read-only in `doen edit`. |
| `date` | ISO date | `-w`, `doen date` | The day that the item is about. For a note, this day is the day of the meeting or of the journal entry. For a todo, this day is the day of capture or the day that is related to the todo. The default value is the day of creation. The `-w` option, sorting and `doen today` use this field. This field never hides or schedules a todo. |
| `scheduled` | ISO date | `-S`, `doen schedule` | Todos only. Optional. The day to start the todo. `doen today` shows an open todo from this day. Separate from `due`, the day by which the todo must be done. |
| `due` | ISO date | `-d`, `doen due` | Todos only. Optional. |
| `priority` | `1`, `2` or `3` | `-p`, `doen priority` | Todos only. Optional. `3` is the highest. |
| `tags` | list of quoted strings | `-t`, `doen tag` | doen always writes this field, even if the list is empty. |
| `archived` | ISO date | `doen archive`, `done`, `drop` | Absent if the item is not archived. Decides the directory. |
| `trashed` | ISO date | `doen rm` | Absent if the item is not in the trash. Decides the directory and the day of permanent deletion. |
| `status` | list of maps | `done`, `drop`, `reopen` | The status log. Each entry has a `date` key and a `status` key (`open`, `done` or `drop`). The entries are in date order. Todos only. Absent until the first status change. |

### Examples

A note:

```yaml
---
id: "a7q2"
kind: note
created: 2026-10-16T09:02:11+11:00
date: 2026-10-16
tags: ["work"]
---

# standup
stuff about it
```

An open todo:

```yaml
---
id: "k3f9"
kind: todo
created: 2026-10-16T21:58:03+11:00
date: 2026-10-16
due: 2026-10-18
tags: ["admin"]
---

# [ ] call bank
stuff about it
```

A todo that had the status `drop`, then `open`, then `done`. The status log is the history. The last entry is the current status:

```yaml
# .archive/20261001-p81c-tax-return.md
---
id: "p81c"
kind: todo
created: 2026-10-01T08:15:40+10:00
date: 2026-10-01
due: 2026-10-13
tags: ["admin"]
archived: 2026-10-16
status:
  - date: 2026-10-05
    status: drop
  - date: 2026-10-09
    status: open
  - date: 2026-10-16
    status: done
---

# [x] tax return
stuff about it
```

An archived note:

```yaml
# .archive/20260912-c0d1-old-meeting-notes.md
---
id: "c0d1"
kind: note
created: 2026-09-12T14:30:00+10:00
date: 2026-09-12
tags: []
archived: 2026-10-16
---

# old meeting notes
stuff about it
```

## Templates, config and tooling

### Templates

Templates are Markdown files. doen looks for templates first in `<store>/.templates/` and then in the global templates directory. The global templates directory is the `templates_dir` config key, or `$XDG_CONFIG_HOME/doen/templates/` if this key is not set. If a store template and a global template have the same name, the store template takes precedence.

- The option `-T meeting` uses the file `meeting.md`.
- If the file `note.md` exists, doen uses this file as the default template for notes.
- If the file `todo.md` exists, doen uses this file as the default template for todos.
- The option `-T none` disables the default template.

A template can contain frontmatter:

- doen merges the `tags` field of the template with the `-t` tags and the `default_tags` tags.
- The `-S` option overrides the `scheduled` field of the template. The `-d` option overrides the `due` field of the template.
- The `scheduled` field and the `due` field can contain a relative date (`due: "+2d"`). doen evaluates the relative date when doen creates the item.
- The `-p` option overrides the `priority` field of the template.
- If you use a template with a `scheduled` field, a `due` field or a `priority` field for a note, doen shows an error.
- doen copies unknown fields into the new item.
- The fields that doen manages (`id`, `kind`, `created`, `date`, `archived`, `trashed`, `status`) are errors in a template.

The body of a template can contain these variables: `{{title}}`, `{{date}}` (ISO format), `{{weekday}}` (`mon` ... `sun`) and `{{id}}`. doen does not change any other `{{...}}` text.

1. doen replaces the variables.
2. The first H1 heading of the template becomes the heading and the title of the item.
3. If the template has no H1 heading, doen inserts `# {{title}}` at the top.
4. For a todo, doen adds the `[ ]` checkbox. Thus, do not write a checkbox in a template.

```yaml
---
tags: ["work", "meeting"]
---
# {{title}}

## attendees

## notes

## actions
```

### Config

doen reads the global config file from `$DOEN_CONFIG` if this variable is set. Otherwise, doen reads `$XDG_CONFIG_HOME/doen/config.yaml`. If `XDG_CONFIG_HOME` is not set, doen reads `~/.config/doen/config.yaml`.

Then doen reads the store config file `<store>/.doen.yaml`. A key in the store config file replaces the key with the same name in the global config file. doen replaces lists. doen does not merge lists.

Precedence, highest first:

1. Command-line options
2. `DOEN_*` environment variables
3. Store config file
4. Global config file
5. General environment variables (`VISUAL`, `EDITOR`, `NO_COLOR`)
6. Built-in defaults

Examples:

- `editor: hx` in a config file takes precedence over `$EDITOR`.
- `color: always` in a config file takes precedence over `NO_COLOR`.
- The `--color` option takes precedence over both.

| Key | Default | Meaning |
| --- | --- | --- |
| `editor` | `$VISUAL`, `$EDITOR`, `vi` | The editor command. |
| `date_format` | `%y-%m-%d` | The display format for dates, as a strftime format. |
| `week_start` | `mon` | The first day of the week for `W`, `LW` and `eow`. The values are `mon` ... `sun`. |
| `relative_days` | `7` | The number of days N. doen shows a date as a relative date if the date is within N days of today. |
| `default_tags` | `[]` | The tags for every new item. |
| `default_sort` | not set | The `--sort` field for both groups. If this key is not set, each kind uses its default order (see Output options). |
| `archive_closed` | `true` | If `true`, `done` and `drop` also archive the todo. |
| `trash_days` | `30` | The number of days N. doen deletes an item permanently N days after the item goes to the trash. The value `0` disables permanent deletion. |
| `git_autocommit` | `false` | If `true`, doen makes a Git commit after every mutating command (see Git). |
| `color` | `auto` | The color mode: `auto`, `always` or `never`. |
| `ls_header` | `true` | If `false`, `doen ls` does not print the banner (see Header and footer). |
| `ls_footer` | `true` | If `false`, `doen ls` does not print the cheatsheet (see Header and footer). |
| `templates_dir` | `$XDG_CONFIG_HOME/doen/templates/` | The global templates directory (see Templates). A leading `~/` is the home directory. A relative path is relative to the store. |
| `ui_keys` | see below | The keys of `doen ui`, by action. |

#### `ui_keys`

`ui_keys` maps action names to a key or a list of keys. An action that is not in the mapping keeps its default keys. An empty list unbinds the action. Unlike other keys, `ui_keys` in the store config file changes only the actions it names, and it has no `DOEN_*` environment variable. An unknown action name gives a warning; an invalid key is an error.

A key is one character (`j`, `G`, `#`), a name (`space`, `enter`, `esc`, `tab`, `backtab` or `shift-tab`, `backspace`, `delete`, `insert`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`), or `f1` to `f12`, with optional `ctrl-` and `alt-` prefixes (`ctrl-r`, `alt-x`). Shift is part of the character: write `G`, not `shift-g`. Quote keys that YAML reads as syntax, such as `"#"`, `"?"` and `"["`.

```yaml
ui_keys:
  quit: [q, ctrl-q]
  toggle_done: [space, c]
  delete: D
  help: f1
```

When two actions share a key, the global actions win over the actions of the focused panel. For example, `enter` is both `submit` (in prompts and the search) and `edit` (in the item list).

| Action | Default keys | Meaning |
| --- | --- | --- |
| `quit` | `q` | Quit. |
| `help` | `?` | Show the keybindings. |
| `focus_items` | `1` | Focus the items. |
| `focus_preview` | `2` | Focus the preview. |
| `next_panel` | `tab` `backtab` | Switch items ↔ preview. |
| `toggle_preview` | `p` | Show or hide the preview. |
| `focus_left` | `h` `left` | Focus the items. |
| `focus_right` | `l` `right` | Focus the preview. |
| `back` | `esc` | Leave the preview, clear the search, leave a list, cancel a prompt. |
| `submit` | `enter` | Accept a prompt or the search. |
| `confirm` | `y` `enter` | Answer yes to a question. |
| `search` | `/` | Search the titles and bodies. |
| `command` | `:` | Run a doen command, as on the command line. |
| `reload` | `ctrl-r` | Reload the store. |
| `new_note` | `n` | New note. |
| `new_todo` | `t` | New todo. |
| `down` | `j` `down` | Move down. |
| `up` | `k` `up` | Move up. |
| `top` | `g` `home` | Go to the first entry. |
| `bottom` | `G` `end` | Go to the last entry. |
| `page_down` | `pagedown` | Move down a page. |
| `page_up` | `pageup` | Move up a page. |
| `edit` | `e` `enter` | Open in the editor. |
| `toggle_done` | `space` | Toggle done. |
| `toggle_drop` | `x` | Toggle dropped. |
| `rename` | `r` | Change the title. |
| `schedule` | `s` | Set the scheduled date. |
| `due` | `u` | Set the due date. |
| `date` | `w` | Set the date. |
| `priority_up` | `+` `=` | Raise the priority. |
| `priority_down` | `-` | Lower the priority. |
| `tags` | `#` | Edit the tags. |
| `archive` | `a` | Archive or unarchive. |
| `delete` | `d` | Move to the trash; in the trash, delete permanently. |
| `restore` | `R` | Restore from the trash. |
| `empty_trash` | `E` | Empty the trash. |
| `preview_down` | `J` | Scroll the preview down. |
| `preview_up` | `K` | Scroll the preview up. |
| `preview_page_down` | `ctrl-d` | Scroll the preview down a page. |
| `preview_page_up` | `ctrl-u` | Scroll the preview up a page. |

### Environment

| Variable | Meaning |
| --- | --- |
| `DOEN_NOTES_DIR` | The location of the store. The default value is `$HOME/doen/`. The `--dir` option overrides this variable. |
| `DOEN_CONFIG` | The path of the global config file. doen uses this path instead of the XDG location. |
| `VISUAL`, `EDITOR` | The editor fallbacks. These variables have lower precedence than the `editor` config key. |
| `NO_COLOR` | If this variable is set, doen disables color. The `color` config key and the `--color` option override this variable. |

### Init

`doen init` creates the store and an empty `.templates/` directory in the store. The location of the store comes from `--dir`, from `$DOEN_NOTES_DIR` or from the default location.

If no global config file exists, `doen init` creates one. In this file, every key has its default value and is commented out. `doen init` does not run `git init`.

### Integrity

- Writes are atomic. doen first writes to a temporary file with the name `.doen-tmp-*` in the store. doen then renames the temporary file. doen ignores these temporary files. Add the pattern `.doen-tmp-*` to `.gitignore` and to the ignore lists of synchronization tools.
- Each mutating command gets a lock on `<store>/.doen.lock`. If another doen process has the lock, the command waits a maximum of 5 seconds for that process to finish.
- The lock does not protect against other computers that synchronize the same store. `doen check` finds the results of these conflicts (duplicate IDs, files in the wrong directory).
- If doen cannot parse a file, doen skips the file and prints a warning on stderr. An invalid file never stops a listing.
- `doen check` reports these problems:
  - Invalid frontmatter and invalid tags
  - Duplicate IDs
  - Filenames that do not match the ID
  - Files in the wrong directory for their `archived` field and `trashed` field
  - Heading checkboxes that disagree with the status log
  - Missing titles
- `doen check --fix` moves the files that are in the wrong directory, rewrites checkboxes from the status log and gives new IDs to duplicates. The command reports invalid frontmatter and missing titles. The command does not fix these two problems.

### Git

Automatic commits occur if `git_autocommit` is `true` and the store is inside a Git work tree. The store can be a subdirectory of a larger repository. In this configuration, every mutating command commits the changed files. doen commits the changes from `doen edit` after the editor closes.

- The commit message is the output line of the command with the prefix `doen:`. For example: `doen: done k3f9 call bank`.
- For create commands, the commit message is `doen: <kind> <id> <title>`.
- If a command changes more than one item, the commit message has more than one line.
- doen also commits automatic permanent deletions from the trash and the changes of `doen check --fix`.
- doen commits only the paths that doen changed (`git commit --only -- <paths>`). Thus, doen never includes unrelated work in a commit. This rule also applies to changes that were already staged.
- doen stages a move into or out of `.archive/` or `.trash/` as a deletion and an addition. Git shows these moves as renames if the content is sufficiently similar.
- If the store is not in a Git repository, or if the commit fails, doen prints a warning on every mutating command and continues. The command itself succeeds.
- doen never pushes and never pulls.

### Shell completion

`doen completion bash|zsh|fish` prints a completion script. The script completes commands, options, IDs (with the titles as descriptions), tags, template names and date keywords.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. This code includes a list with no results and a command that only printed warnings because the values did not change. |
| `1` | General error. |
| `2` | Usage error: an invalid option, an invalid date, a missing argument, an unknown command, or a command on the wrong kind of item. |
| `3` | An ID is not found or is ambiguous. |
| `4` | The user declined a confirmation, or a confirmation was necessary but no terminal was available. |
| `5` | `grep` found no matches. |

## Scope and decisions

### Out of scope by design

- **Recurring todos, subtasks and dependencies:** These features require a scheduler and a graph. doen keeps a flat list.
- **Times of day:** doen uses only dates. The exception is the `created` field.
- **Synchronization:** The store contains only plain files. Use Git, Syncthing or a similar tool. `git_autocommit` records history but never pushes.
- **Links between notes:** Plain Markdown links work. doen does not track links.
- **Inline tags:** Tags are only in the frontmatter.

### Deferred

- **Deadline dates:** A `deadline` date is a hard deadline that is separate from `due`. org-mode has this concept. Review this feature after version 1. The `due` field leaves room to add this field without a migration.

### Decisions

| Question | Decision |
| --- | --- |
| Frontmatter format | YAML with `---` delimiters, for compatibility with Obsidian and most Markdown tools. Config files also use YAML. |
| ID scheme | A random 4-character base36 ID, stored in the frontmatter and in the filename. |
| Scheduled dates | A `scheduled` field, separate from `due`. `doen today` shows an open todo from its scheduled day. |
| Deadline dates | Deferred until after version 1. |
| Archive storage | The `.archive/` directory follows the `archived` field. The frontmatter is the source of truth. |
| Trash storage | The `.trash/` directory follows the `trashed` field. The age for permanent deletion comes from this field, not from the modification time (mtime) of the file. |
| Git integration | The `git_autocommit` config key. The default value is `false`. |
| Inline `#tags` in the body | Not supported. |
| Stdin for content | doen reads stdin only with `-c -`, never automatically. |
| Commands with more than one ID | All or nothing. If one ID is invalid, doen changes no items. |
| Checkbox that is edited by hand | doen treats the change as a status command if the status log did not also change. Otherwise, the status log takes precedence. |
| Config precedence | Command-line options, then `DOEN_*` environment variables, then the store config file, then the global config file, then general environment variables. |

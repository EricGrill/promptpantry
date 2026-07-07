# Prompt Pantry (`pp`)

Prompt Pantry is a local, git-backed prompt library for people who reuse,
adapt, and share prompts from the terminal.

Prompts live as plain Markdown files. `pp` adds a fast TUI, a scriptable CLI,
template variables, clipboard copy, and git sync around that folder. There is no
hosted service and no database to migrate.

## Features

- Markdown prompt cards with optional YAML frontmatter
- Fuzzy search across prompt titles, tags, and paths
- `{{variable}}` rendering with built-in repo, branch, cwd, and date values
- Terminal UI for browsing, previewing, creating, editing, copying, and syncing
- CLI commands for scripts and shell workflows
- Git-backed prompt library with focused commits for local changes
- Optional `library.yaml` catalog for reusable prompts, skills, and agents
- `pp doctor` health check for frontmatter, ambiguous titles, and catalog integrity

## Install

Requirements:

- Rust toolchain
- `git`
- Clipboard support for `pp copy` outside `--stdout`

Install from a clone:

```sh
git clone https://github.com/EricGrill/promptpantry.git
cd promptpantry
cargo install --path .
```

Install directly from GitHub:

```sh
cargo install --git https://github.com/EricGrill/promptpantry.git
```

There is no crates.io release yet, so install from source or GitHub.

The binary is named `pp`. If another program named `pp` is earlier in your
`PATH`, move Cargo's bin directory earlier or call the installed binary by its
full path.

Linux note: on X11/Wayland without a clipboard manager, copied text may not
survive after `pp` exits. macOS and Windows clipboard behavior is unaffected.

## Quick Start

```sh
pp init
pp
pp list
pp show bug report
pp show bug report --var ticket=ABC-123
pp copy bug report --var ticket=ABC-123
pp new "evals/Rubric Writer" --tags evals,writing
pp sync
```

`pp` with no subcommand opens the TUI. Use `pp show` or `pp view` to print a
prompt without touching the clipboard. Multi-word queries can be typed naturally:

```sh
pp show bug report
```

Tag queries should usually be quoted so the shell does not treat `#` as a
comment:

```sh
pp list '#bugs'
```

## Prompt Cards

A prompt card is a Markdown file in your prompt library. Frontmatter is optional.

```markdown
---
title: Bug Report Template
tags: [bugs, templates]
description: Structured repro report
---
Repo: {{repo}}
Ticket: {{ticket}}

## Steps to reproduce
1.

## Expected

## Actual
```

Everything after the frontmatter is the prompt body. If frontmatter is missing,
the title falls back to the filename. If frontmatter is malformed, the card still
loads and the TUI shows the parse error in the preview.

Built-in variables are filled from the directory where you run `pp`:

| Variable | Value |
| --- | --- |
| `{{repo}}` | Current git repository directory name |
| `{{branch}}` | Current git branch |
| `{{cwd}}` | Current working directory |
| `{{date}}` | Current date as `YYYY-MM-DD` |

Any other variable can be filled in the TUI form or supplied with repeatable
`--var key=value` flags.

## CLI

```sh
pp init
pp list [query...] [--json]
pp show [query...] [--id id] [--raw] [--var key=value] [--json]
pp view [query...] [--id id] [--raw] [--var key=value] [--json]
pp copy [query...] [--id id] [--raw] [--stdout] [--var key=value]
pp new <title> [--tags tag,tag]
pp sync
pp doctor [--json]
pp library <command>
```

`--json` on `list`, `show`, and `doctor` emits machine-readable output for
scripting (pipe it to `jq`): `list` yields an array of
`{id, title, tags, description}`, `show` a single object with the rendered
`body`, and `doctor` a `{findings: [...]}` object (still exits non-zero on errors).

| Command | Purpose |
| --- | --- |
| `pp init` | Create the prompt library, initialize git, and add an example card |
| `pp list [query...]` | Print `id<TAB>title<TAB>tag,tag` rows |
| `pp show [query...]` | Print a prompt; placeholders stay intact unless `--var` is supplied |
| `pp view [query...]` | Alias for `pp show` |
| `pp copy [query...]` | Render a prompt and copy it to the clipboard |
| `pp copy --stdout ...` | Render a prompt and print it instead of using the clipboard |
| `pp new <title>` | Create a card and open it in `$EDITOR`, then `$VISUAL`, then `vi` |
| `pp sync` | Commit external edits, run `git pull --rebase`, then push |
| `pp doctor` | Check the library and catalog for problems; exits non-zero on errors |

Search notes:

- Query words are fuzzy-matched against title, tags, and path.
- `#tag` tokens filter by tag prefix, for example `pp list '#bug'`.
- `--id` selects an exact card id such as `evals/rubric-writer`.
- `--raw` keeps placeholders unchanged.

## TUI

Run `pp` to open the terminal UI.

Prompt-card view:

| Key | Action |
| --- | --- |
| type | Fuzzy-search titles, tags, and paths |
| Tab | Switch to the capability catalog |
| Up/Down or `^k`/`^j` | Move selection |
| Enter | Copy selected card, opening the variable form when needed |
| `^n` | Create a new card in `$EDITOR` |
| `^e` | Edit selected card in `$EDITOR` |
| `^d` | Delete selected card after confirmation |
| `^s` | Run `pp sync` for the prompt library |
| PgUp/PgDn | Scroll preview |
| Esc | Clear query, then quit |
| `^c` | Quit |

Capability-catalog view:

| Key | Action |
| --- | --- |
| type | Search catalog entries by name or description |
| Tab | Switch back to prompt cards |
| Up/Down or `^k`/`^j` | Move selection |
| Enter | Install selected entry and its dependencies |
| `a` | Add a catalog entry |
| `i` | Import a `library.yaml` catalog |
| `^s` | Refresh installed catalog entries from their sources |
| `^p` | Confirm and push local edits back to the selected entry's source |
| `^d` | Remove selected catalog entry and local installs after confirmation |
| PgUp/PgDn | Scroll preview |
| Esc | Clear query, then quit |
| `^c` | Quit |

## Library Location

Prompt Pantry resolves the prompt library directory in this order:

1. `--dir <path>`
2. `PROMPT_PANTRY_DIR`
3. `dir` in `~/.config/prompt-pantry/config.toml`
4. `~/prompts`

`pp init` creates the directory, initializes git, writes a README, and seeds an
example card. Create, edit, delete, and catalog operations write files first and
then attempt focused git commits. If git fails after the file operation
succeeds, `pp` reports a warning instead of discarding your content.

## Capability Catalog

The optional catalog lives at `<prompt-library>/library.yaml`. It tracks reusable
prompts, skills, and agents that can be installed into project-local or global
`.claude/*` directories on demand.

Common commands:

```sh
pp library import /path/to/library.yaml
pp library add prompt bug --description "Bug prompt" --source /path/to/bug.md
pp library add skill reviewer \
  --description "Review code" \
  --source /path/to/reviewer/SKILL.md \
  --requires prompt:bug
pp library list
pp library search review
pp library use reviewer
pp library use reviewer --global
pp library use reviewer --target /tmp/capabilities
pp library sync
pp library push reviewer
pp library remove reviewer --delete-local
```

Default install targets:

| Kind | Project-local target | Global target |
| --- | --- | --- |
| prompt | `.claude/commands/<name>.md` | `~/.claude/commands/<name>.md` |
| skill | `.claude/skills/<name>/` | `~/.claude/skills/<name>/` |
| agent | `.claude/agents/<name>.md` | `~/.claude/agents/<name>.md` |

Supported source formats:

- Absolute local paths
- `~/...` local paths
- GitHub file URLs such as
  `https://github.com/org/repo/blob/main/path/to/SKILL.md`
- Raw GitHub URLs such as
  `https://raw.githubusercontent.com/org/repo/main/path/to/SKILL.md`

Skills install by copying the directory containing `SKILL.md`. Prompts and
agents install as single Markdown files. Dependencies are typed as `kind:name`,
for example `prompt:bug` or `skill:reviewer`, and are installed before the
requested entry.

Prompt Pantry rejects catalog names and GitHub source paths that try to traverse
outside their install or clone directories. Skill installs intentionally copy the
whole directory containing `SKILL.md`, so keep private notes, fixtures,
credentials, and build artifacts outside that directory before installing or
pushing a skill.

## Security and Privacy

- Prompt cards are local files in a git repository you control.
- `pp sync` uses that repository's configured git remote.
- Catalog commands fetch from or push to GitHub only when the selected catalog
  source points at GitHub and you run the relevant command.
- No telemetry or hosted service is used.
- Security reporting guidance lives in [SECURITY.md](SECURITY.md).

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The test suite covers parsing, search, template rendering, git-backed library
operations, CLI flows, catalog install/sync/push/remove/import behavior, and TUI
rendering/state-machine smoke tests.

## License

No open-source license has been selected yet. Until a license is added, the code
is publicly visible but not open-source licensed for reuse.

# Prompt Pantry (`pp`)

Prompt Pantry is a local, git-backed prompt library with a fast terminal UI and
scriptable CLI. It stores reusable prompts as plain markdown cards, fills
`{{variables}}`, previews or copies rendered prompts, and auto-commits library
changes so the prompt collection can be synced like any other git repo.

It also manages a `library.yaml` capability catalog for reusable prompts,
skills, and agents. Catalog entries can point at local files or GitHub file
URLs, then install into project-local or global `.claude/*` directories on
demand.

## Install

```sh
cargo install --path .
```

Requirements:

- Rust toolchain
- `git` for library initialization, auto-commits, sync, and catalog GitHub
  sources
- Clipboard support for `pp copy` outside `--stdout`

Linux note: on X11/Wayland without a clipboard manager, copied text may not
survive after `pp` exits. macOS and Windows are unaffected.

## Quick Start

```sh
pp init
pp
pp list
pp show bug report --var ticket=ABC-123
pp copy bug report --var ticket=ABC-123 --stdout
pp new "evals/Rubric Writer" --tags evals,writing
pp sync
```

`pp` with no subcommand opens the TUI. Press Tab to switch between prompt cards
and the capability catalog. Use `pp show` (or `pp view`) to print a prompt
without touching the clipboard. Multi-word search queries can be passed as
separate words, so shell quotes are optional.

## Prompt Cards

Cards are markdown files with optional YAML frontmatter:

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
```

The rendered body is everything after the frontmatter. Missing frontmatter is
allowed; the card title falls back to the filename. Malformed frontmatter keeps
the card loadable and surfaces the parse error in the preview.

Built-in variables are pre-filled from the directory where you run `pp`:

| Variable | Value |
|---|---|
| `{{repo}}` | current git repository directory name |
| `{{branch}}` | current git branch |
| `{{cwd}}` | current working directory |
| `{{date}}` | current date as `YYYY-MM-DD` |

Any other variable is requested in the TUI form or supplied with repeatable
`--var key=value` flags in the CLI.

## CLI

```sh
pp init
pp list [query...]
pp show [query...] [--id id] [--raw] [--var key=value]
pp view [query...] [--id id] [--raw] [--var key=value]
pp copy [query...] [--id id] [--raw] [--stdout] [--var key=value]
pp new <title> [--tags tag,tag]
pp sync
```

Notes:

- `pp list` prints `id<TAB>title<TAB>tag,tag`.
- `#tag` query tokens filter by tag prefix, e.g. `pp list '#bugs'`.
- `pp show` prints to stdout and leaves placeholders intact unless `--var` is
  supplied.
- `pp copy` copies to the clipboard by default; `--stdout` prints instead.
- `pp new` opens `$EDITOR`, then `$VISUAL`, then `vi`.
- `pp sync` commits external prompt-library edits, runs `git pull --rebase`,
  then pushes.

## TUI Keys

Prompt-card view:

| Key | Action |
|---|---|
| type | fuzzy-search titles, tags, and paths |
| Tab | switch to the capability catalog |
| Up/Down or `^k`/`^j` | move selection |
| Enter | copy selected card, opening the variable form when needed |
| `^n` | create a new card in `$EDITOR` |
| `^e` | edit selected card in `$EDITOR` |
| `^d` | delete selected card after confirmation |
| `^s` | run `pp sync` for the prompt library |
| PgUp/PgDn | scroll preview |
| Esc | clear query, then quit |
| `^c` | quit |

Capability-catalog view:

| Key | Action |
|---|---|
| type | search catalog entries by name or description |
| Tab | switch back to prompt cards |
| Up/Down or `^k`/`^j` | move selection |
| Enter | install selected entry and its dependencies |
| `a` | add a catalog entry |
| `i` | import a `library.yaml` catalog |
| `^s` | refresh installed catalog entries from their sources |
| `^p` | confirm and push local edits back to the selected entry's source |
| `^d` | remove selected catalog entry and local installs after confirmation |
| PgUp/PgDn | scroll preview |
| Esc | clear query, then quit |
| `^c` | quit |

## Library Location

Prompt Pantry resolves the library directory in this order:

1. `--dir <path>`
2. `PROMPT_PANTRY_DIR`
3. `dir` in `~/.config/prompt-pantry/config.toml`
4. `~/prompts`

`pp init` creates the directory, initializes git, writes a README, and seeds an
example card. Every create, edit, delete, and catalog update writes files first
and then attempts a focused git commit. Git failures are reported as warnings
when the content operation itself succeeded.

## Capability Catalog

The capability catalog lives at `<prompt-library>/library.yaml`. It records
available prompts, skills, and agents; nothing is installed until you use an
entry. The same catalog can be managed from the CLI or from the TUI's catalog
view.

```sh
pp library import /path/to/library.yaml
pp library add prompt bug --description "Bug command" --source /path/to/bug.md
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
|---|---|---|
| prompt | `.claude/commands/<name>.md` | `~/.claude/commands/<name>.md` |
| skill | `.claude/skills/<name>/` | `~/.claude/skills/<name>/` |
| agent | `.claude/agents/<name>.md` | `~/.claude/agents/<name>.md` |

Supported source formats:

- absolute local paths
- `~/...` local paths
- GitHub browser file URLs such as
  `https://github.com/org/repo/blob/main/path/to/SKILL.md`
- raw GitHub URLs such as
  `https://raw.githubusercontent.com/org/repo/main/path/to/SKILL.md`

Skills install by copying the directory containing `SKILL.md`. Prompts and
agents install as single markdown files. `pp library sync` refreshes installed
entries from their sources. `pp library push` copies local edits back to a local
source, or commits and pushes back to a GitHub source when the source URL points
at GitHub.

Catalog entry names are plain file names, not paths. Prompt Pantry rejects names
and GitHub source paths that try to traverse outside their install or clone
directories. Skill installs intentionally copy the whole directory containing
`SKILL.md`, so keep private notes, fixtures, credentials, and build artifacts
outside that directory before installing or pushing a skill.

Dependencies are typed as `kind:name`, for example `prompt:bug` or
`skill:reviewer`, and are installed before the requested entry.

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The test suite covers core parsing/search/template behavior, git-backed library
operations, CLI flows, catalog install/sync/push/remove/import behavior, and TUI
rendering/state-machine smoke tests.

Security policy and reporting guidance live in `SECURITY.md`. This repository is
public, but it does not declare an open-source license yet; until a license is
added, reuse rights are limited to GitHub's standard public-repository terms.

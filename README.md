# Prompt Pantry (`pp`)

A local, git-backed TUI for storing and reusing AI prompts — agent prompts,
system prompts, eval prompts, bug templates. Cards are plain markdown files
with YAML frontmatter in `~/prompts` (a git repo). Fuzzy-search them, preview,
fill `{{variables}}`, and copy to the clipboard.

## Install

    cargo install --path .

Linux note: on X11/Wayland without a clipboard manager, copied text may not
survive after `pp` exits (macOS/Windows unaffected).

## Quick start

    pp init          # create ~/prompts, git init, seed an example card
    pp               # open the TUI: type to search, ↵ to copy
    pp list          # id<TAB>title<TAB>tags — pipe it anywhere
    pp copy "bug report" --var ticket=ABC-123 --stdout
    pp new "evals/Rubric Writer" --tags evals
    pp sync          # commit externals, pull --rebase, push

## TUI keys

| Key | Action |
|---|---|
| type | fuzzy-search titles, tags, paths (`#tag` filters by tag) |
| ↑/↓ or ^k/^j | move selection |
| ↵ | copy card (opens variable form if it has `{{vars}}`) |
| ^n | new card (opens $EDITOR) |
| ^e | edit selected card |
| ^d | delete selected card (confirm) |
| PgUp/PgDn | scroll preview |
| esc | clear query, then quit; ^c always quits |

## Card format

    ---
    title: Bug Report Template
    tags: [bugs, templates]
    description: optional one-liner
    ---
    Repo: {{repo}}
    Ticket: {{ticket}}

Builtins pre-filled from where you run `pp`: `{{repo}}`, `{{branch}}`,
`{{cwd}}`, `{{date}}`. Everything else is asked for in the form.

## Library location

`--dir` flag > `PROMPT_PANTRY_DIR` env > `dir` in
`~/.config/prompt-pantry/config.toml` > `~/prompts` (default).

Every create/edit/delete through `pp` is auto-committed. Push/pull with
`pp sync` or plain git — it's your repo.

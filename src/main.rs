use clap::{Parser, Subcommand};
use prompt_pantry::{cli, core::config, tui};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pp",
    version,
    about = "Prompt Pantry — local git-backed prompt library"
)]
struct Cli {
    /// Library directory (overrides PROMPT_PANTRY_DIR and the config file)
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the library folder, git repo, README and an example card
    Init,
    /// List cards as `id<TAB>title<TAB>tag,tag` (same query syntax as the TUI)
    List { query: Option<String> },
    /// Render a card and copy it to the clipboard
    Copy {
        /// Fuzzy query; best match wins
        #[arg(conflicts_with = "id")]
        query: Option<String>,
        /// Exact card id (e.g. evals/rubric-writer)
        #[arg(long)]
        id: Option<String>,
        /// Variable value, repeatable: --var ticket=ABC-123
        #[arg(long = "var", value_name = "KEY=VALUE", conflicts_with = "raw")]
        vars: Vec<String>,
        /// Copy with {{placeholders}} intact
        #[arg(long)]
        raw: bool,
        /// Print to stdout instead of copying
        #[arg(long)]
        stdout: bool,
    },
    /// Create a new card and open it in $EDITOR ('/' in the title creates subfolders)
    New {
        title: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Commit pending changes, then git pull --rebase && git push
    Sync,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let dir = config::resolve_library_dir(args.dir);
    match args.cmd {
        None => tui::run(dir),
        Some(Cmd::Init) => cli::init::run(&dir),
        Some(Cmd::List { query }) => cli::list::run(&dir, query.as_deref().unwrap_or("")),
        Some(Cmd::Copy {
            query,
            id,
            vars,
            raw,
            stdout,
        }) => cli::copy::run(&dir, query.as_deref(), id.as_deref(), &vars, raw, stdout),
        Some(Cmd::New { title, tags }) => cli::new::run(&dir, &title, &tags),
        Some(Cmd::Sync) => cli::sync::run(&dir),
    }
}

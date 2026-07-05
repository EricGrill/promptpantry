use clap::{Parser, Subcommand};
use prompt_pantry::{cli, core::config, tui};
use std::path::PathBuf;

const HELP_EXAMPLES: &str = "\
Examples:
  pp init
  pp list
  pp list '#bugs'
  pp show bug report
  pp show --id bug-report-template --raw
  pp show --id bug-report-template --var ticket=ABC-123
  pp copy bug report --var ticket=ABC-123
  pp new \"evals/Rubric Writer\" --tags evals,writing
  pp sync";

#[derive(Parser)]
#[command(
    name = "pp",
    version,
    about = "Prompt Pantry — local git-backed prompt library",
    after_help = HELP_EXAMPLES
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
    List {
        /// Fuzzy query words; tags still work, e.g. '#bugs'
        #[arg(value_name = "QUERY")]
        query: Vec<String>,
    },
    /// Print a prompt to stdout (placeholders stay intact unless --var is supplied)
    #[command(visible_alias = "view")]
    Show {
        /// Fuzzy query; best match wins
        #[arg(value_name = "QUERY", conflicts_with = "id")]
        query: Vec<String>,
        /// Exact card id (e.g. evals/rubric-writer)
        #[arg(long)]
        id: Option<String>,
        /// Variable value, repeatable: --var ticket=ABC-123
        #[arg(long = "var", value_name = "KEY=VALUE", conflicts_with = "raw")]
        vars: Vec<String>,
        /// Print with {{placeholders}} intact (default when no --var is supplied)
        #[arg(long)]
        raw: bool,
    },
    /// Render a prompt and copy it to the clipboard
    Copy {
        /// Fuzzy query; best match wins
        #[arg(value_name = "QUERY", conflicts_with = "id")]
        query: Vec<String>,
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

fn query_text(parts: Vec<String>) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let dir = config::resolve_library_dir(args.dir);
    match args.cmd {
        None => tui::run(dir),
        Some(Cmd::Init) => cli::init::run(&dir),
        Some(Cmd::List { query }) => cli::list::run(&dir, &query.join(" ")),
        Some(Cmd::Show {
            query,
            id,
            vars,
            raw,
        }) => {
            let query = query_text(query);
            let raw = raw || vars.is_empty();
            cli::copy::run(&dir, query.as_deref(), id.as_deref(), &vars, raw, true)
        }
        Some(Cmd::Copy {
            query,
            id,
            vars,
            raw,
            stdout,
        }) => {
            let query = query_text(query);
            cli::copy::run(&dir, query.as_deref(), id.as_deref(), &vars, raw, stdout)
        }
        Some(Cmd::New { title, tags }) => cli::new::run(&dir, &title, &tags),
        Some(Cmd::Sync) => cli::sync::run(&dir),
    }
}

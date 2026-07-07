use crate::core::{context, search::search, store::Store, template};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::Path;

pub fn run(
    dir: &Path,
    query: Option<&str>,
    id: Option<&str>,
    var_args: &[String],
    raw: bool,
    to_stdout: bool,
    json_out: bool,
) -> Result<()> {
    let store = Store::open(dir.to_path_buf())?;
    let cards = store.load_cards();
    let card = if let Some(id) = id {
        cards
            .iter()
            .find(|c| c.id == id)
            .with_context(|| format!("no card with id `{id}`"))?
    } else {
        let q = query.context("give a query or --id (see `pp list`)")?;
        *search(&cards, q)
            .first()
            .with_context(|| format!("no card matches `{q}` — try `pp list`"))?
    };

    let text = if raw {
        card.body.clone()
    } else {
        let mut values = context::builtin_values(&std::env::current_dir()?);
        for kv in var_args {
            let (k, v) = kv
                .split_once('=')
                .with_context(|| format!("--var must be KEY=VALUE, got `{kv}`"))?;
            values.insert(k.to_string(), v.to_string());
        }
        let names = template::extract_vars(&card.body);
        let missing: Vec<String> = names
            .iter()
            .filter(|n| !values.contains_key(*n))
            .cloned()
            .collect();
        if !missing.is_empty() {
            bail!(
                "missing variables: {} (use --var name=value)",
                missing.join(", ")
            );
        }
        template::render(&card.body, &values)
    };

    if json_out {
        let out = json!({
            "id": card.id,
            "title": card.title,
            "tags": card.tags,
            "description": card.description,
            "raw": raw,
            "body": text,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if to_stdout {
        print!("{text}");
        return Ok(());
    }
    // On X11/Wayland without a clipboard manager, content set by a process is
    // dropped when it exits — macOS/Windows are fine. OSC52 is the noted future fix.
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
        Ok(()) => eprintln!("copied `{}` to clipboard", card.id),
        Err(e) => {
            eprintln!("warning: clipboard unavailable ({e}); printing instead");
            print!("{text}");
        }
    }
    Ok(())
}

use crate::core::{search::search, store::Store};
use anyhow::Result;
use serde_json::json;
use std::path::Path;

pub fn run(dir: &Path, query: &str, json_out: bool) -> Result<()> {
    let store = Store::open(dir.to_path_buf())?;
    let cards = store.load_cards();
    let results = search(&cards, query);

    if json_out {
        let rows: Vec<_> = results
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "title": c.title,
                    "tags": c.tags,
                    "description": c.description,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    for c in results {
        println!("{}\t{}\t{}", c.id, c.title, c.tags.join(","));
    }
    Ok(())
}

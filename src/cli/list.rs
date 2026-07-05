use crate::core::{search::search, store::Store};
use anyhow::Result;
use std::path::Path;

pub fn run(dir: &Path, query: &str) -> Result<()> {
    let store = Store::open(dir.to_path_buf())?;
    let cards = store.load_cards();
    for c in search(&cards, query) {
        println!("{}\t{}\t{}", c.id, c.title, c.tags.join(","));
    }
    Ok(())
}

use crate::core::gitops;
use anyhow::Result;
use std::path::Path;

pub fn run(dir: &Path) -> Result<()> {
    println!("{}", gitops::sync(dir)?.trim());
    Ok(())
}

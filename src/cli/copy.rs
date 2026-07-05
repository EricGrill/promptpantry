use anyhow::Result;
use std::path::Path;

pub fn run(
    _dir: &Path,
    _query: Option<&str>,
    _id: Option<&str>,
    _vars: &[String],
    _raw: bool,
    _stdout: bool,
) -> Result<()> {
    anyhow::bail!("not implemented yet")
}

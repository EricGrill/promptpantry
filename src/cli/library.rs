use crate::core::{
    catalog::{self, CatalogEntry, EntryKind, InstallScope},
    store::Store,
};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn add(
    dir: &Path,
    kind: EntryKind,
    name: String,
    description: String,
    source: String,
    requires: Vec<String>,
) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    catalog::add(dir, kind, name.clone(), description, source, requires)?;
    println!("added {} `{name}`", kind.as_str());
    Ok(())
}

pub fn import(dir: &Path, source: &Path) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    let count = catalog::import(dir, source)?;
    println!("imported {count} library entries");
    Ok(())
}

pub fn list(dir: &Path) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    print_rows(dir, None)
}

pub fn search(dir: &Path, query: &str) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    print_rows(dir, Some(query))
}

fn print_rows(dir: &Path, query: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    for row in catalog::rows(dir, query, &cwd)? {
        print_entry(row.kind, &row.entry, &row.status);
    }
    Ok(())
}

fn print_entry(kind: EntryKind, entry: &CatalogEntry, status: &str) {
    println!(
        "{}\t{}\t{}\t{}\t{}",
        kind.as_str(),
        entry.name,
        entry.description,
        entry.source,
        status
    );
}

pub fn use_entry(dir: &Path, query: &str, global: bool, target: Option<PathBuf>) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    let scope = install_scope(global, target);
    let cwd = std::env::current_dir()?;
    for report in catalog::use_entry(dir, query, scope, &cwd)? {
        println!(
            "{}\t{}\t{}",
            report.kind.as_str(),
            report.name,
            report.status
        );
    }
    Ok(())
}

pub fn sync(dir: &Path) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    let cwd = std::env::current_dir()?;
    for report in catalog::sync_installed(dir, &cwd)? {
        println!(
            "{}\t{}\t{}",
            report.kind.as_str(),
            report.name,
            report.status
        );
    }
    Ok(())
}

pub fn push(dir: &Path, query: &str) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    let cwd = std::env::current_dir()?;
    let report = catalog::push_entry(dir, query, &cwd)?;
    println!(
        "{}\t{}\t{}",
        report.kind.as_str(),
        report.name,
        report.status
    );
    Ok(())
}

pub fn remove(dir: &Path, query: &str, delete_local: bool) -> Result<()> {
    Store::open(dir.to_path_buf())?;
    let cwd = std::env::current_dir()?;
    let report = catalog::remove(dir, query, delete_local, &cwd)?;
    println!(
        "{}\t{}\t{}",
        report.kind.as_str(),
        report.name,
        report.status
    );
    Ok(())
}

fn install_scope(global: bool, target: Option<PathBuf>) -> InstallScope {
    match (global, target) {
        (_, Some(path)) => InstallScope::Custom(path),
        (true, None) => InstallScope::Global,
        (false, None) => InstallScope::Default,
    }
}

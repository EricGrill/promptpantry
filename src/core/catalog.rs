use crate::core::gitops;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CATALOG_FILE: &str = "library.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Skill,
    Agent,
    Prompt,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Skill => "skill",
            EntryKind::Agent => "agent",
            EntryKind::Prompt => "prompt",
        }
    }

    fn typed_key(self, name: &str) -> String {
        format!("{}:{name}", self.as_str())
    }
}

impl std::str::FromStr for EntryKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "skill" | "skills" => Ok(EntryKind::Skill),
            "agent" | "agents" => Ok(EntryKind::Agent),
            "prompt" | "prompts" | "command" | "commands" => Ok(EntryKind::Prompt),
            _ => bail!("unknown library kind `{s}`"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: String,
    pub description: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogFileData {
    #[serde(default)]
    pub default_dirs: DefaultDirs,
    #[serde(default)]
    pub library: LibrarySections,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultDirs {
    #[serde(default = "default_skill_dirs")]
    pub skills: Vec<BTreeMap<String, String>>,
    #[serde(default = "default_agent_dirs")]
    pub agents: Vec<BTreeMap<String, String>>,
    #[serde(default = "default_prompt_dirs")]
    pub prompts: Vec<BTreeMap<String, String>>,
}

impl Default for DefaultDirs {
    fn default() -> Self {
        Self {
            skills: default_skill_dirs(),
            agents: default_agent_dirs(),
            prompts: default_prompt_dirs(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibrarySections {
    #[serde(default)]
    pub skills: Vec<CatalogEntry>,
    #[serde(default)]
    pub agents: Vec<CatalogEntry>,
    #[serde(default)]
    pub prompts: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRow {
    pub kind: EntryKind,
    pub entry: CatalogEntry,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReport {
    pub kind: EntryKind,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum InstallScope {
    Default,
    Global,
    Custom(PathBuf),
}

fn dir_entry(key: &str, value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(key.to_string(), value.to_string())])
}

fn default_skill_dirs() -> Vec<BTreeMap<String, String>> {
    vec![
        dir_entry("default", ".claude/skills/"),
        dir_entry("global", "~/.claude/skills/"),
    ]
}

fn default_agent_dirs() -> Vec<BTreeMap<String, String>> {
    vec![
        dir_entry("default", ".claude/agents/"),
        dir_entry("global", "~/.claude/agents/"),
    ]
}

fn default_prompt_dirs() -> Vec<BTreeMap<String, String>> {
    vec![
        dir_entry("default", ".claude/commands/"),
        dir_entry("global", "~/.claude/commands/"),
    ]
}

pub fn catalog_path(dir: &Path) -> PathBuf {
    dir.join(CATALOG_FILE)
}

pub fn load(dir: &Path) -> Result<CatalogFileData> {
    let path = catalog_path(dir);
    if !path.exists() {
        return Ok(CatalogFileData::default());
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let catalog: CatalogFileData =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    validate_catalog_entries(&catalog)?;
    Ok(catalog)
}

pub fn save(dir: &Path, catalog: &CatalogFileData) -> Result<()> {
    fs::create_dir_all(dir)?;
    let raw = serde_yaml::to_string(catalog)?;
    fs::write(catalog_path(dir), raw)?;
    Ok(())
}

fn commit_catalog_change(dir: &Path, message: &str) {
    if gitops::is_repo(dir) && gitops::has_changes(dir, CATALOG_FILE).unwrap_or(false) {
        if let Err(e) = gitops::commit_path(dir, CATALOG_FILE, message) {
            eprintln!("warning: catalog commit failed: {e}");
        }
    }
}

fn entries(catalog: &CatalogFileData, kind: EntryKind) -> &[CatalogEntry] {
    match kind {
        EntryKind::Skill => &catalog.library.skills,
        EntryKind::Agent => &catalog.library.agents,
        EntryKind::Prompt => &catalog.library.prompts,
    }
}

fn entries_mut(catalog: &mut CatalogFileData, kind: EntryKind) -> &mut Vec<CatalogEntry> {
    match kind {
        EntryKind::Skill => &mut catalog.library.skills,
        EntryKind::Agent => &mut catalog.library.agents,
        EntryKind::Prompt => &mut catalog.library.prompts,
    }
}

fn all_entries(catalog: &CatalogFileData) -> Vec<(EntryKind, CatalogEntry)> {
    [EntryKind::Skill, EntryKind::Agent, EntryKind::Prompt]
        .into_iter()
        .flat_map(|kind| {
            entries(catalog, kind)
                .iter()
                .cloned()
                .map(move |entry| (kind, entry))
        })
        .collect()
}

pub fn add(
    dir: &Path,
    kind: EntryKind,
    name: String,
    description: String,
    source: String,
    requires: Vec<String>,
) -> Result<()> {
    validate_catalog_name(&name).with_context(|| format!("invalid {} name", kind.as_str()))?;
    for dep in &requires {
        validate_dependency(dep)?;
    }
    validate_source(&source)?;
    let mut catalog = load(dir)?;
    let section = entries_mut(&mut catalog, kind);
    if section.iter().any(|e| e.name == name) {
        bail!(
            "{} `{name}` already exists in library catalog",
            kind.as_str()
        );
    }
    section.push(CatalogEntry {
        name,
        description,
        source,
        requires,
    });
    section.sort_by_key(|e| e.name.to_lowercase());
    save(dir, &catalog)?;
    commit_catalog_change(dir, "pp: update library catalog");
    Ok(())
}

pub fn import(dir: &Path, source: &Path) -> Result<usize> {
    let source = if source.is_dir() {
        source.join(CATALOG_FILE)
    } else {
        source.to_path_buf()
    };
    let raw =
        fs::read_to_string(&source).with_context(|| format!("reading {}", source.display()))?;
    let imported: CatalogFileData =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", source.display()))?;
    validate_catalog_entries(&imported)?;
    for (kind, entry) in all_entries(&imported) {
        parse_source(&entry.source)
            .with_context(|| format!("invalid source for {} `{}`", kind.as_str(), entry.name))?;
    }
    let mut catalog = load(dir)?;
    let mut changed = 0;
    for (kind, entry) in all_entries(&imported) {
        upsert(entries_mut(&mut catalog, kind), entry);
        changed += 1;
    }
    for kind in [EntryKind::Skill, EntryKind::Agent, EntryKind::Prompt] {
        entries_mut(&mut catalog, kind).sort_by_key(|e| e.name.to_lowercase());
    }
    save(dir, &catalog)?;
    commit_catalog_change(dir, "pp: import library catalog");
    Ok(changed)
}

fn upsert(section: &mut Vec<CatalogEntry>, entry: CatalogEntry) {
    if let Some(existing) = section.iter_mut().find(|e| e.name == entry.name) {
        *existing = entry;
    } else {
        section.push(entry);
    }
}

pub fn rows(dir: &Path, query: Option<&str>, cwd: &Path) -> Result<Vec<CatalogRow>> {
    let catalog = load(dir)?;
    let needle = query.map(str::to_lowercase);
    let mut rows = Vec::new();
    for (kind, entry) in all_entries(&catalog) {
        if let Some(needle) = &needle {
            let hay = format!("{} {}", entry.name, entry.description).to_lowercase();
            if !hay.contains(needle) {
                continue;
            }
        }
        rows.push(CatalogRow {
            status: install_status(&catalog, kind, &entry.name, cwd),
            kind,
            entry,
        });
    }
    rows.sort_by_key(|row| (row.kind.as_str(), row.entry.name.to_lowercase()));
    Ok(rows)
}

pub fn use_entry(
    dir: &Path,
    query: &str,
    scope: InstallScope,
    cwd: &Path,
) -> Result<Vec<ActionReport>> {
    let catalog = load(dir)?;
    let (kind, entry) = find_unique(&catalog, query)?;
    let mut visited = HashSet::new();
    let mut reports = Vec::new();
    install_recursive(
        &catalog,
        kind,
        &entry.name,
        &scope,
        cwd,
        &mut visited,
        &mut reports,
    )?;
    Ok(reports)
}

pub fn sync_installed(dir: &Path, cwd: &Path) -> Result<Vec<ActionReport>> {
    let catalog = load(dir)?;
    let mut reports = Vec::new();
    for (kind, entry) in all_entries(&catalog) {
        for scope in installed_scopes(&catalog, kind, &entry.name, cwd) {
            let mut visited = HashSet::new();
            install_recursive(
                &catalog,
                kind,
                &entry.name,
                &scope,
                cwd,
                &mut visited,
                &mut reports,
            )?;
        }
    }
    if reports.is_empty() {
        reports.push(ActionReport {
            kind: EntryKind::Prompt,
            name: "library".to_string(),
            status: "no installed catalog items".to_string(),
        });
    }
    Ok(reports)
}

pub fn push_entry(dir: &Path, query: &str, cwd: &Path) -> Result<ActionReport> {
    let catalog = load(dir)?;
    let (kind, entry) = find_unique(&catalog, query)?;
    let installed = installed_path(&catalog, kind, &entry.name, cwd)
        .with_context(|| format!("{} `{}` is not installed", kind.as_str(), entry.name))?;
    match parse_source(&entry.source)? {
        Source::Local(source_file) => push_to_local_source(kind, &installed, &source_file)?,
        Source::Github(source) => push_to_github_source(kind, &entry.name, &installed, &source)?,
    }
    Ok(ActionReport {
        kind,
        name: entry.name,
        status: "pushed".to_string(),
    })
}

pub fn remove(dir: &Path, query: &str, delete_local: bool, cwd: &Path) -> Result<ActionReport> {
    let mut catalog = load(dir)?;
    let (kind, entry) = find_unique(&catalog, query)?;
    entries_mut(&mut catalog, kind).retain(|e| e.name != entry.name);
    save(dir, &catalog)?;
    commit_catalog_change(dir, "pp: update library catalog");
    if delete_local {
        for path in [
            target_item_path(
                kind,
                &target_base(&catalog, kind, &InstallScope::Default, cwd)?,
                &entry.name,
            ),
            target_item_path(
                kind,
                &target_base(&catalog, kind, &InstallScope::Global, cwd)?,
                &entry.name,
            ),
        ] {
            remove_path_if_exists(&path)?;
        }
    }
    Ok(ActionReport {
        kind,
        name: entry.name,
        status: if delete_local {
            "removed and deleted local installs".to_string()
        } else {
            "removed from catalog".to_string()
        },
    })
}

fn find_unique(catalog: &CatalogFileData, query: &str) -> Result<(EntryKind, CatalogEntry)> {
    let exact: Vec<_> = all_entries(catalog)
        .into_iter()
        .filter(|(_, entry)| entry.name == query)
        .collect();
    if exact.len() == 1 {
        return Ok(exact.into_iter().next().unwrap());
    }
    let needle = query.to_lowercase();
    let matches: Vec<_> = all_entries(catalog)
        .into_iter()
        .filter(|(_, entry)| {
            entry.name.to_lowercase().contains(&needle)
                || entry.description.to_lowercase().contains(&needle)
        })
        .collect();
    match matches.len() {
        0 => bail!("no library catalog entry matches `{query}`"),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!("multiple library catalog entries match `{query}`"),
    }
}

fn find_exact(catalog: &CatalogFileData, kind: EntryKind, name: &str) -> Result<CatalogEntry> {
    entries(catalog, kind)
        .iter()
        .find(|entry| entry.name == name)
        .cloned()
        .with_context(|| {
            format!(
                "dependency {} not found in library catalog",
                kind.typed_key(name)
            )
        })
}

fn install_recursive(
    catalog: &CatalogFileData,
    kind: EntryKind,
    name: &str,
    scope: &InstallScope,
    cwd: &Path,
    visited: &mut HashSet<String>,
    reports: &mut Vec<ActionReport>,
) -> Result<()> {
    let key = kind.typed_key(name);
    if !visited.insert(key) {
        return Ok(());
    }
    let entry = find_exact(catalog, kind, name)?;
    for dep in &entry.requires {
        let (dep_kind, dep_name) = parse_dependency(dep)?;
        install_recursive(catalog, dep_kind, dep_name, scope, cwd, visited, reports)?;
    }
    let base = target_base(catalog, kind, scope, cwd)?;
    fetch_to_target(
        kind,
        &entry.source,
        &target_item_path(kind, &base, &entry.name),
    )?;
    reports.push(ActionReport {
        kind,
        name: entry.name,
        status: format!("installed to {}", base.display()),
    });
    Ok(())
}

fn parse_dependency(dep: &str) -> Result<(EntryKind, &str)> {
    let (kind, name) = dep
        .split_once(':')
        .with_context(|| format!("dependency `{dep}` must be typed, e.g. skill:name"))?;
    Ok((kind.parse()?, name))
}

fn install_status(catalog: &CatalogFileData, kind: EntryKind, name: &str, cwd: &Path) -> String {
    let default = target_base(catalog, kind, &InstallScope::Default, cwd)
        .ok()
        .map(|base| target_item_path(kind, &base, name).exists())
        .unwrap_or(false);
    let global = target_base(catalog, kind, &InstallScope::Global, cwd)
        .ok()
        .map(|base| target_item_path(kind, &base, name).exists())
        .unwrap_or(false);
    match (default, global) {
        (true, true) => "installed (default, global)".to_string(),
        (true, false) => "installed (default)".to_string(),
        (false, true) => "installed (global)".to_string(),
        (false, false) => "not installed".to_string(),
    }
}

fn installed_scopes(
    catalog: &CatalogFileData,
    kind: EntryKind,
    name: &str,
    cwd: &Path,
) -> Vec<InstallScope> {
    [InstallScope::Default, InstallScope::Global]
        .into_iter()
        .filter(|scope| {
            target_base(catalog, kind, scope, cwd)
                .map(|base| target_item_path(kind, &base, name).exists())
                .unwrap_or(false)
        })
        .collect()
}

fn installed_path(
    catalog: &CatalogFileData,
    kind: EntryKind,
    name: &str,
    cwd: &Path,
) -> Option<PathBuf> {
    for scope in [InstallScope::Default, InstallScope::Global] {
        let base = target_base(catalog, kind, &scope, cwd).ok()?;
        let path = target_item_path(kind, &base, name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn target_dirs(catalog: &CatalogFileData, kind: EntryKind) -> &[BTreeMap<String, String>] {
    match kind {
        EntryKind::Skill => &catalog.default_dirs.skills,
        EntryKind::Agent => &catalog.default_dirs.agents,
        EntryKind::Prompt => &catalog.default_dirs.prompts,
    }
}

fn target_base(
    catalog: &CatalogFileData,
    kind: EntryKind,
    scope: &InstallScope,
    cwd: &Path,
) -> Result<PathBuf> {
    match scope {
        InstallScope::Custom(path) => Ok(resolve_path(path, cwd)),
        InstallScope::Default | InstallScope::Global => {
            let key = match scope {
                InstallScope::Default => "default",
                InstallScope::Global => "global",
                InstallScope::Custom(_) => unreachable!(),
            };
            let raw = target_dirs(catalog, kind)
                .iter()
                .find_map(|m| m.get(key))
                .with_context(|| format!("no {key} target dir configured for {}", kind.as_str()))?;
            Ok(resolve_path(Path::new(raw), cwd))
        }
    }
}

fn resolve_path(path: &Path, cwd: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return dirs::home_dir().unwrap_or_else(|| cwd.to_path_buf());
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return dirs::home_dir()
            .unwrap_or_else(|| cwd.to_path_buf())
            .join(rest);
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn target_item_path(kind: EntryKind, base: &Path, name: &str) -> PathBuf {
    debug_assert!(validate_catalog_name(name).is_ok());
    match kind {
        EntryKind::Skill => base.join(name),
        EntryKind::Agent | EntryKind::Prompt => base.join(format!("{name}.md")),
    }
}

#[derive(Debug)]
enum Source {
    Local(PathBuf),
    Github(GithubSource),
}

#[derive(Debug)]
struct GithubSource {
    clone_url: String,
    ssh_url: String,
    branch: String,
    file_path: PathBuf,
}

fn validate_source(source: &str) -> Result<()> {
    match parse_source(source)? {
        Source::Local(path) => {
            if !path.is_file() {
                bail!("source file does not exist: {}", path.display());
            }
        }
        Source::Github(_) => {}
    }
    Ok(())
}

fn validate_catalog_entries(catalog: &CatalogFileData) -> Result<()> {
    for (kind, entry) in all_entries(catalog) {
        validate_catalog_name(&entry.name)
            .with_context(|| format!("invalid {} name `{}`", kind.as_str(), entry.name))?;
        for dep in &entry.requires {
            validate_dependency(dep).with_context(|| {
                format!(
                    "invalid dependency `{dep}` for {} `{}`",
                    kind.as_str(),
                    entry.name
                )
            })?;
        }
    }
    Ok(())
}

fn validate_catalog_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("catalog names cannot be empty");
    }
    if name.trim() != name {
        bail!("catalog names cannot start or end with whitespace");
    }
    if name == "." || name == ".." {
        bail!("catalog names cannot be `.` or `..`");
    }
    if name.contains('/') || name.contains('\\') {
        bail!("catalog names cannot contain path separators");
    }
    if name.chars().any(|c| c.is_control()) {
        bail!("catalog names cannot contain control characters");
    }
    let path = Path::new(name);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("catalog names must be plain file names");
    }
    Ok(())
}

fn validate_dependency(dep: &str) -> Result<()> {
    let (kind, name) = dep
        .split_once(':')
        .with_context(|| format!("dependency `{dep}` must be typed, e.g. skill:name"))?;
    let _: EntryKind = kind.parse()?;
    validate_catalog_name(name)
}

fn parse_source(source: &str) -> Result<Source> {
    if source.starts_with('/') || source.starts_with("~/") || source == "~" {
        return Ok(Source::Local(resolve_path(
            Path::new(source),
            &std::env::current_dir()?,
        )));
    }
    if let Some(rest) = source.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 5 && parts[2] == "blob" {
            let org = parts[0];
            let repo = parts[1];
            validate_github_slug(org, "owner")?;
            validate_github_slug(repo, "repository")?;
            let branch = parts[3].to_string();
            validate_github_branch(&branch)?;
            let file_path = github_file_path(&parts[4..])?;
            return Ok(Source::Github(GithubSource {
                clone_url: format!("https://github.com/{org}/{repo}.git"),
                ssh_url: format!("git@github.com:{org}/{repo}.git"),
                branch,
                file_path,
            }));
        }
    }
    if let Some(rest) = source.strip_prefix("https://raw.githubusercontent.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 4 {
            let org = parts[0];
            let repo = parts[1];
            validate_github_slug(org, "owner")?;
            validate_github_slug(repo, "repository")?;
            let branch = parts[2].to_string();
            validate_github_branch(&branch)?;
            let file_path = github_file_path(&parts[3..])?;
            return Ok(Source::Github(GithubSource {
                clone_url: format!("https://github.com/{org}/{repo}.git"),
                ssh_url: format!("git@github.com:{org}/{repo}.git"),
                branch,
                file_path,
            }));
        }
    }
    bail!("unsupported source `{source}`; use a local file path or GitHub file URL")
}

fn validate_github_slug(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value == "." || value == ".." {
        bail!("GitHub {label} cannot be `{value}`");
    }
    if value.contains('\\') || value.chars().any(|c| c.is_control()) {
        bail!("GitHub {label} contains invalid characters");
    }
    Ok(())
}

fn validate_github_branch(branch: &str) -> Result<()> {
    if branch.is_empty() || branch.chars().any(|c| c.is_control()) {
        bail!("GitHub branch contains invalid characters");
    }
    Ok(())
}

fn github_file_path(parts: &[&str]) -> Result<PathBuf> {
    if parts.is_empty() {
        bail!("GitHub source URL must include a file path");
    }
    let mut path = PathBuf::new();
    for part in parts {
        if part.is_empty() || *part == "." || *part == ".." {
            bail!("GitHub source file path cannot contain `{part}`");
        }
        if part.contains('\\') || part.chars().any(|c| c.is_control()) {
            bail!("GitHub source file path contains invalid characters");
        }
        path.push(part);
    }
    Ok(path)
}

fn fetch_to_target(kind: EntryKind, source: &str, target: &Path) -> Result<()> {
    match parse_source(source)? {
        Source::Local(path) => fetch_local(kind, &path, target),
        Source::Github(source) => {
            let tmp = clone_source(&source)?;
            let source_file = tmp.join(&source.file_path);
            let result = fetch_local(kind, &source_file, target);
            let _ = fs::remove_dir_all(tmp);
            result
        }
    }
}

fn fetch_local(kind: EntryKind, source_file: &Path, target: &Path) -> Result<()> {
    if !source_file.is_file() {
        bail!("source file does not exist: {}", source_file.display());
    }
    match kind {
        EntryKind::Skill => {
            let source_dir = source_file
                .parent()
                .with_context(|| format!("source has no parent: {}", source_file.display()))?;
            copy_dir(source_dir, target)
        }
        EntryKind::Agent | EntryKind::Prompt => copy_file(source_file, target),
    }
}

fn push_to_local_source(kind: EntryKind, installed: &Path, source_file: &Path) -> Result<()> {
    match kind {
        EntryKind::Skill => {
            let source_dir = source_file
                .parent()
                .with_context(|| format!("source has no parent: {}", source_file.display()))?;
            copy_dir(installed, source_dir)
        }
        EntryKind::Agent | EntryKind::Prompt => copy_file(installed, source_file),
    }
}

fn push_to_github_source(
    kind: EntryKind,
    name: &str,
    installed: &Path,
    source: &GithubSource,
) -> Result<()> {
    let tmp = clone_source(source)?;
    let dest_file = tmp.join(&source.file_path);
    match kind {
        EntryKind::Skill => {
            let dest_dir = dest_file
                .parent()
                .with_context(|| format!("source has no parent: {}", dest_file.display()))?;
            copy_dir(installed, dest_dir)?;
        }
        EntryKind::Agent | EntryKind::Prompt => copy_file(installed, &dest_file)?,
    }
    let rel_path = match kind {
        EntryKind::Skill => source
            .file_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        EntryKind::Agent | EntryKind::Prompt => source.file_path.clone(),
    };
    let rel = rel_path.to_string_lossy().to_string();
    run_git(&tmp, &["add", "-A", "--", &rel])?;
    run_git(
        &tmp,
        &[
            "commit",
            "-m",
            &format!("pp: update library {} {name}", kind.as_str()),
        ],
    )?;
    run_git(&tmp, &["push"])?;
    let _ = fs::remove_dir_all(tmp);
    Ok(())
}

fn clone_source(source: &GithubSource) -> Result<PathBuf> {
    let tmp = unique_temp_dir("pp-library-clone")?;
    let https = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            &source.branch,
            &source.clone_url,
            tmp.to_str().unwrap(),
        ])
        .output()
        .context("failed to run git clone")?;
    if https.status.success() {
        return Ok(tmp);
    }
    let _ = fs::remove_dir_all(&tmp);
    let ssh = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            &source.branch,
            &source.ssh_url,
            tmp.to_str().unwrap(),
        ])
        .output()
        .context("failed to run git clone via ssh")?;
    if ssh.status.success() {
        return Ok(tmp);
    }
    let _ = fs::remove_dir_all(&tmp);
    bail!(
        "git clone failed: {}",
        String::from_utf8_lossy(&https.stderr).trim()
    )
}

fn run_git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn unique_temp_dir(prefix: &str) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn copy_file(source: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)
        .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
    Ok(())
}

fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    remove_path_if_exists(target)?;
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source).with_context(|| format!("reading {}", source.display()))? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_dir(&source_path, &target_path)?;
        } else if kind.is_file() {
            copy_file(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("checking {}", path.display())),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn local_prompt(tmp: &TempDir) -> PathBuf {
        let source = tmp.path().join("source.md");
        fs::write(&source, "prompt body\n").unwrap();
        source
    }

    #[test]
    fn add_rejects_catalog_names_that_escape_install_targets() {
        let tmp = TempDir::new().unwrap();
        let source = local_prompt(&tmp);

        let err = add(
            tmp.path(),
            EntryKind::Prompt,
            "../escaped".to_string(),
            "bad".to_string(),
            source.to_string_lossy().into_owned(),
            vec![],
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid prompt name"));
        assert!(!tmp.path().join("library.yaml").exists());
    }

    #[test]
    fn import_rejects_catalog_names_that_escape_install_targets() {
        let tmp = TempDir::new().unwrap();
        let source = local_prompt(&tmp);
        let imported = tmp.path().join("import.yaml");
        fs::write(
            &imported,
            format!(
                "library:\n  prompts:\n    - name: ../escaped\n      description: bad\n      source: {}\n",
                source.display()
            ),
        )
        .unwrap();

        let err = import(tmp.path(), &imported).unwrap_err();

        assert!(err.to_string().contains("invalid prompt name"));
        assert!(!tmp.path().join("library.yaml").exists());
    }

    #[test]
    fn load_rejects_catalog_dependencies_that_escape_install_targets() {
        let tmp = TempDir::new().unwrap();
        let source = local_prompt(&tmp);
        fs::write(
            tmp.path().join("library.yaml"),
            format!(
                "library:\n  prompts:\n    - name: writer\n      description: bad dependency\n      source: {}\n      requires:\n        - prompt:../escaped\n",
                source.display()
            ),
        )
        .unwrap();

        let err = load(tmp.path()).unwrap_err();

        assert!(err.to_string().contains("invalid dependency"));
    }

    #[test]
    fn github_source_paths_cannot_escape_the_clone_dir() {
        let err =
            parse_source("https://github.com/org/repo/blob/main/../../private.md").unwrap_err();
        assert!(err.to_string().contains("cannot contain `..`"), "{err:#}");

        let err = parse_source("https://raw.githubusercontent.com/org/repo/main/./private.md")
            .unwrap_err();
        assert!(err.to_string().contains("cannot contain `.`"), "{err:#}");
    }

    #[test]
    fn github_source_paths_accept_normal_nested_files() {
        let Source::Github(source) =
            parse_source("https://github.com/org/repo/blob/main/skills/reviewer/SKILL.md").unwrap()
        else {
            panic!("expected GitHub source");
        };

        assert_eq!(source.branch, "main");
        assert_eq!(
            source.file_path,
            PathBuf::from("skills").join("reviewer").join("SKILL.md")
        );
    }
}

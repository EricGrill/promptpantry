use crate::core::card::{parse_card, Card};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    /// Open an existing library directory. Errors (with a `pp init` hint) if missing.
    pub fn open(root: PathBuf) -> Result<Store> {
        if !root.is_dir() {
            bail!("library not found at {} — run `pp init` to create it", root.display());
        }
        Ok(Store { root })
    }

    /// All cards, recursive, sorted by title (case-insensitive).
    /// Skips `.git` and the root README.md (written by `pp init`, not a card).
    pub fn load_cards(&self) -> Vec<Card> {
        let mut cards: Vec<Card> = WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "md"))
            .filter_map(|e| {
                let rel = e.path().strip_prefix(&self.root).ok()?;
                if rel == Path::new("README.md") {
                    return None;
                }
                let id = rel.with_extension("").to_string_lossy().replace('\\', "/");
                let raw = fs::read_to_string(e.path()).ok()?;
                Some(parse_card(id, e.path().to_path_buf(), &raw))
            })
            .collect();
        cards.sort_by_key(|c| c.title.to_lowercase());
        cards
    }

    /// Create a card file from a title (`/` maps to subdirectories). Never overwrites.
    /// Returns (id, absolute path).
    pub fn create_card(&self, title: &str, tags: &[String]) -> Result<(String, PathBuf)> {
        let id = kebab(title);
        if id.is_empty() || id.split('/').any(|s| s.is_empty()) {
            bail!("title `{title}` produces an invalid filename");
        }
        let path = self.root.join(format!("{id}.md"));
        if path.exists() {
            bail!("card already exists: {}", path.display());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let display_title = title.rsplit('/').next().unwrap_or(title).trim().replace('"', "\\\"");
        let tags_line = if tags.is_empty() {
            String::new()
        } else {
            format!("tags: [{}]\n", tags.join(", "))
        };
        let content = format!("---\ntitle: \"{display_title}\"\n{tags_line}---\n\n");
        fs::write(&path, &content).with_context(|| format!("writing {}", path.display()))?;
        Ok((id, path))
    }

    pub fn delete_card(&self, card: &Card) -> Result<()> {
        fs::remove_file(&card.path).with_context(|| format!("deleting {}", card.path.display()))
    }
}

/// `Evals/Rubric Writer!` -> `evals/rubric-writer`
pub fn kebab(s: &str) -> String {
    s.split('/')
        .map(|part| {
            part.to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn lib_with(files: &[(&str, &str)]) -> (Store, TempDir) {
        let tmp = TempDir::new().unwrap();
        for (rel, content) in files {
            let p = tmp.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
        }
        (Store::open(tmp.path().to_path_buf()).unwrap(), tmp)
    }

    #[test]
    fn open_missing_dir_errors_with_init_hint() {
        let err = Store::open(PathBuf::from("/nonexistent/pantry")).unwrap_err();
        assert!(err.to_string().contains("pp init"));
    }

    #[test]
    fn loads_recursively_sorted_skipping_readme_and_non_md() {
        let (store, _tmp) = lib_with(&[
            ("zebra.md", "---\ntitle: Zebra\n---\nz"),
            ("sub/alpha.md", "---\ntitle: alpha\n---\na"),
            ("README.md", "# not a card"),
            ("notes.txt", "not markdown"),
        ]);
        let cards = store.load_cards();
        let ids: Vec<&str> = cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["sub/alpha", "zebra"]);
    }

    #[test]
    fn create_card_kebabs_supports_subdirs_never_overwrites() {
        let (store, _tmp) = lib_with(&[]);
        let (id, path) = store.create_card("Evals/Rubric Writer!", &["evals".into()]).unwrap();
        assert_eq!(id, "evals/rubric-writer");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("title: \"Rubric Writer!\""));
        assert!(content.contains("tags: [evals]"));
        assert!(store.create_card("Evals/Rubric Writer!", &[]).is_err());
    }

    #[test]
    fn delete_card_removes_the_file() {
        let (store, _tmp) = lib_with(&[("gone.md", "---\ntitle: Gone\n---\nbye")]);
        let card = store.load_cards().remove(0);
        store.delete_card(&card).unwrap();
        assert!(!card.path.exists());
    }

    #[test]
    fn kebab_cases() {
        assert_eq!(kebab("Hello,  World!"), "hello-world");
        assert_eq!(kebab("evals/Rubric Writer"), "evals/rubric-writer");
    }
}

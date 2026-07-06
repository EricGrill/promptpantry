use serde::Deserialize;
use std::path::PathBuf;

/// One prompt card = one markdown file in the library.
#[derive(Debug, Clone)]
pub struct Card {
    /// Relative path inside the library, without `.md` (e.g. `evals/rubric-writer`).
    pub id: String,
    /// Absolute path to the file.
    pub path: PathBuf,
    pub title: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
    /// Everything after the frontmatter — what gets rendered and copied.
    pub body: String,
    /// Set when frontmatter exists but fails to parse (`!` badge in the TUI).
    pub parse_error: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FrontMatter {
    title: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
}

/// `dir/my-card` -> `my card`
pub fn title_from_id(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).replace('-', " ")
}

/// Normalize line endings to \n for cross-platform frontmatter detection.
fn normalize_line_endings(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Split raw content into (frontmatter yaml, body). None when there is no frontmatter block.
fn split_front_matter(raw: &str) -> Option<(String, String)> {
    let normalized = normalize_line_endings(raw);
    let rest = normalized.strip_prefix("---\n")?;
    if let Some(idx) = rest.find("\n---\n") {
        let body_start = idx + 5;
        return Some((rest[..idx].to_string(), rest[body_start..].to_string()));
    }
    // frontmatter block that ends at EOF
    rest.strip_suffix("\n---")
        .map(|fm| (fm.to_string(), String::new()))
}

pub fn parse_card(id: String, path: PathBuf, raw: &str) -> Card {
    let (title, tags, description, body, parse_error) = match split_front_matter(raw) {
        None => (title_from_id(&id), vec![], None, raw.to_string(), None),
        Some((fm_raw, body)) => match serde_yaml::from_str::<FrontMatter>(&fm_raw) {
            Ok(fm) => (
                fm.title.unwrap_or_else(|| title_from_id(&id)),
                fm.tags.unwrap_or_default(),
                fm.description,
                body,
                None,
            ),
            Err(e) => (
                title_from_id(&id),
                vec![],
                None,
                body,
                Some(format!("bad frontmatter: {e}")),
            ),
        },
    };
    Card {
        id,
        path,
        title,
        tags,
        description,
        body,
        parse_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(raw: &str) -> Card {
        parse_card("dir/my-card".into(), PathBuf::from("/tmp/x.md"), raw)
    }

    #[test]
    fn parses_full_frontmatter() {
        let c = card("---\ntitle: Bug Report\ntags: [bugs, templates]\ndescription: Repro\n---\nBody {{repo}}\n");
        assert_eq!(c.title, "Bug Report");
        assert_eq!(c.tags, vec!["bugs", "templates"]);
        assert_eq!(c.description.as_deref(), Some("Repro"));
        assert_eq!(c.body, "Body {{repo}}\n");
        assert!(c.parse_error.is_none());
    }

    #[test]
    fn missing_frontmatter_falls_back_to_filename() {
        let c = card("just a body\n");
        assert_eq!(c.title, "my card");
        assert!(c.tags.is_empty());
        assert_eq!(c.body, "just a body\n");
        assert!(c.parse_error.is_none());
    }

    #[test]
    fn malformed_frontmatter_sets_error_and_keeps_body() {
        let c = card("---\ntitle: [unclosed\n---\nbody\n");
        assert!(c.parse_error.is_some());
        assert_eq!(c.title, "my card");
        assert_eq!(c.body, "body\n");
    }

    #[test]
    fn crlf_line_endings_detect_frontmatter() {
        let c = card("---\r\ntitle: Windows Card\r\ntags: [win]\r\n---\r\nBody text\r\n");
        assert_eq!(c.title, "Windows Card");
        assert_eq!(c.tags, vec!["win"]);
        assert_eq!(c.body, "Body text\r\n");
        assert!(c.parse_error.is_none());
    }

    #[test]
    fn bare_cr_line_endings_detect_frontmatter() {
        let c = card("---\rtitle: Old Mac\r---\rBody\r");
        assert_eq!(c.title, "Old Mac");
        assert_eq!(c.body, "Body\r");
        assert!(c.parse_error.is_none());
    }
}

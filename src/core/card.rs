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

/// Split raw content into (frontmatter yaml, body). None when there is no frontmatter block.
fn split_front_matter(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix("---\n")?;
    if let Some(idx) = rest.find("\n---\n") {
        return Some((&rest[..idx], &rest[idx + 5..]));
    }
    // frontmatter block that ends at EOF
    rest.strip_suffix("\n---").map(|fm| (fm, ""))
}

pub fn parse_card(id: String, path: PathBuf, raw: &str) -> Card {
    let (title, tags, description, body, parse_error) = match split_front_matter(raw) {
        None => (title_from_id(&id), vec![], None, raw.to_string(), None),
        Some((fm_raw, body)) => match serde_yaml::from_str::<FrontMatter>(fm_raw) {
            Ok(fm) => (
                fm.title.unwrap_or_else(|| title_from_id(&id)),
                fm.tags.unwrap_or_default(),
                fm.description,
                body.to_string(),
                None,
            ),
            Err(e) => (
                title_from_id(&id),
                vec![],
                None,
                body.to_string(),
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
}

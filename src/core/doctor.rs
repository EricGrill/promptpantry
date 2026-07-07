use crate::core::catalog;
use crate::core::store::Store;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Severity of a `pp doctor` finding. Any error makes the command exit non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A single problem found while inspecting the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub category: &'static str,
    pub message: String,
}

/// The full result of a health check.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }
}

/// Scan the library at `dir` and the `library.yaml` beside it, collecting findings.
/// Reads the filesystem but has no other side effects — safe to unit-test with a tempdir.
pub fn check(dir: &Path) -> Report {
    let mut findings = Vec::new();
    check_cards(dir, &mut findings);
    check_catalog(dir, &mut findings);
    Report { findings }
}

fn error(category: &'static str, message: String) -> Finding {
    Finding {
        severity: Severity::Error,
        category,
        message,
    }
}

fn warning(category: &'static str, message: String) -> Finding {
    Finding {
        severity: Severity::Warning,
        category,
        message,
    }
}

fn check_cards(dir: &Path, findings: &mut Vec<Finding>) {
    let store = match Store::open(dir.to_path_buf()) {
        Ok(store) => store,
        Err(e) => {
            findings.push(error("library", e.to_string()));
            return;
        }
    };
    let cards = store.load_cards();

    for card in &cards {
        if let Some(err) = &card.parse_error {
            findings.push(error("frontmatter", format!("{}: {err}", card.id)));
        }
    }

    // Cards that share a (case-insensitive) title make `pp show <query>` ambiguous.
    let mut by_title: HashMap<String, Vec<String>> = HashMap::new();
    for card in &cards {
        by_title
            .entry(card.title.to_lowercase())
            .or_default()
            .push(card.id.clone());
    }
    let mut clashes: Vec<(String, Vec<String>)> = by_title
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .collect();
    clashes.sort();
    for (title, mut ids) in clashes {
        ids.sort();
        findings.push(warning(
            "duplicate-title",
            format!(
                "{} cards share the title `{title}`: {}",
                ids.len(),
                ids.join(", ")
            ),
        ));
    }
}

fn check_catalog(dir: &Path, findings: &mut Vec<Finding>) {
    let catalog = match catalog::load(dir) {
        Ok(catalog) => catalog,
        Err(e) => {
            findings.push(error("catalog", format!("{e:#}")));
            return;
        }
    };
    let integrity = catalog::integrity_findings(&catalog);
    for message in integrity.dangling {
        findings.push(error("catalog-dependency", message));
    }
    for message in integrity.duplicates {
        findings.push(warning("catalog-duplicate", message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn lib(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        for (rel, content) in files {
            let p = tmp.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, content).unwrap();
        }
        tmp
    }

    fn categories(report: &Report) -> Vec<&str> {
        report.findings.iter().map(|f| f.category).collect()
    }

    #[test]
    fn clean_library_has_no_findings() {
        let tmp = lib(&[("good.md", "---\ntitle: Good\n---\nbody\n")]);
        let report = check(tmp.path());
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        assert_eq!(report.errors(), 0);
    }

    #[test]
    fn malformed_frontmatter_is_an_error() {
        let tmp = lib(&[("bad.md", "---\ntitle: [unterminated\n---\nbody\n")]);
        let report = check(tmp.path());
        assert_eq!(report.errors(), 1);
        assert!(categories(&report).contains(&"frontmatter"));
    }

    #[test]
    fn duplicate_titles_warn_but_do_not_error() {
        let tmp = lib(&[
            ("a.md", "---\ntitle: Same\n---\n"),
            ("sub/b.md", "---\ntitle: same\n---\n"),
        ]);
        let report = check(tmp.path());
        assert_eq!(report.errors(), 0);
        assert_eq!(report.warnings(), 1);
        let msg = &report.findings[0].message;
        assert!(msg.contains("a") && msg.contains("sub/b"), "{msg}");
    }

    #[test]
    fn dangling_catalog_dependency_is_an_error() {
        let tmp = lib(&[(
            "library.yaml",
            "library:\n  prompts:\n    - name: writer\n      description: needs a missing skill\n      source: https://github.com/x/y\n      requires:\n        - skill:ghost\n",
        )]);
        let report = check(tmp.path());
        assert_eq!(report.errors(), 1, "{:?}", report.findings);
        assert!(categories(&report).contains(&"catalog-dependency"));
    }

    #[test]
    fn satisfied_catalog_dependency_is_clean() {
        let tmp = lib(&[(
            "library.yaml",
            "library:\n  skills:\n    - name: helper\n      description: a skill\n      source: https://github.com/x/y\n  prompts:\n    - name: writer\n      description: needs helper\n      source: https://github.com/x/y\n      requires:\n        - skill:helper\n",
        )]);
        let report = check(tmp.path());
        assert!(report.findings.is_empty(), "{:?}", report.findings);
    }

    #[test]
    fn report_serializes_severity_lowercase() {
        let report = Report {
            findings: vec![Finding {
                severity: Severity::Error,
                category: "frontmatter",
                message: "boom".into(),
            }],
        };
        let s = serde_json::to_string(&report).unwrap();
        assert!(s.contains(r#""severity":"error""#), "{s}");
        assert!(s.contains(r#""category":"frontmatter""#), "{s}");
    }

    #[test]
    fn unparseable_catalog_is_a_single_error() {
        let tmp = lib(&[("library.yaml", "library:\n  prompts: : not valid yaml\n")]);
        let report = check(tmp.path());
        assert!(report.errors() >= 1);
        assert!(categories(&report).contains(&"catalog"));
    }
}

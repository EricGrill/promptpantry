use crate::core::card::Card;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, PartialEq)]
pub struct Query {
    pub tags: Vec<String>,
    pub text: String,
}

/// `#bug repro steps` -> tags=[bug], text="repro steps". A bare `#` is ignored.
/// `#tag` tokens filter by case-insensitive PREFIX match against card tags
/// (`#bug` matches `bugs` and `bug-triage`).
pub fn parse_query(raw: &str) -> Query {
    let (mut tags, mut words) = (Vec::new(), Vec::new());
    for tok in raw.split_whitespace() {
        if let Some(t) = tok.strip_prefix('#') {
            if !t.is_empty() {
                tags.push(t.to_lowercase());
            }
        } else {
            words.push(tok);
        }
    }
    Query {
        tags,
        text: words.join(" "),
    }
}

/// Indices into `cards`, tag-filtered (prefix match, see [`parse_query`]) then
/// fuzzy-ranked over "title tags id". The text query goes through
/// `Pattern::parse`, so nucleo/fzf-style operators are supported by design:
/// `!neg` (negation), `^prefix`, `'exact`, `$suffix`.
/// Empty text returns all tag-filtered cards in their given (title-sorted) order.
pub fn search_indices(cards: &[Card], raw: &str) -> Vec<usize> {
    let q = parse_query(raw);
    let candidates: Vec<usize> = (0..cards.len())
        .filter(|&i| {
            q.tags.iter().all(|t| {
                cards[i]
                    .tags
                    .iter()
                    .any(|ct| ct.to_lowercase().starts_with(t))
            })
        })
        .collect();
    if q.text.is_empty() {
        return candidates;
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&q.text, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize)> = candidates
        .into_iter()
        .filter_map(|i| {
            let c = &cards[i];
            let hay = format!("{} {} {}", c.title, c.tags.join(" "), c.id);
            let score = pattern.score(Utf32Str::new(&hay, &mut buf), &mut matcher)?;
            Some((score, i))
        })
        .collect();
    scored.sort_by_key(|&(s, _)| std::cmp::Reverse(s)); // stable: ties keep title order
    scored.into_iter().map(|(_, i)| i).collect()
}

pub fn search<'a>(cards: &'a [Card], raw: &str) -> Vec<&'a Card> {
    search_indices(cards, raw)
        .into_iter()
        .map(|i| &cards[i])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mk(id: &str, title: &str, tags: &[&str]) -> Card {
        Card {
            id: id.into(),
            path: PathBuf::from(format!("/lib/{id}.md")),
            title: title.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            description: None,
            body: String::new(),
            parse_error: None,
        }
    }

    fn sample() -> Vec<Card> {
        vec![
            mk("bug-report", "Bug Report Template", &["bugs", "templates"]),
            mk("evals/rubric-writer", "Rubric Writer", &["evals"]),
            mk("standup", "Standup Notes", &["daily"]),
        ]
    }

    #[test]
    fn parse_query_splits_tags_and_text() {
        let q = parse_query("#bug repro #daily steps");
        assert_eq!(q.tags, vec!["bug", "daily"]);
        assert_eq!(q.text, "repro steps");
    }

    #[test]
    fn tag_tokens_filter_by_prefix() {
        let cards = sample();
        let hits = search(&cards, "#bug");
        assert_eq!(
            hits.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["bug-report"]
        );
    }

    #[test]
    fn fuzzy_matches_title_tags_and_path() {
        let cards = sample();
        assert_eq!(search(&cards, "bugrep")[0].id, "bug-report");
        assert_eq!(search(&cards, "evals rubric")[0].id, "evals/rubric-writer");
        assert!(search(&cards, "zzzzqq").is_empty());
    }

    #[test]
    fn empty_query_returns_all() {
        assert_eq!(search(&sample(), "").len(), 3);
    }
}

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

fn var_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{\{\s*([A-Za-z0-9_-]+)\s*\}\}").unwrap())
}

/// Unique variable names in order of first appearance.
pub fn extract_vars(body: &str) -> Vec<String> {
    let mut seen = Vec::new();
    for cap in var_re().captures_iter(body) {
        let name = cap[1].to_string();
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

/// Replace every {{var}} with its value; unknown vars render as empty strings.
pub fn render(body: &str, values: &HashMap<String, String>) -> String {
    var_re()
        .replace_all(body, |cap: &regex::Captures| {
            values.get(&cap[1]).cloned().unwrap_or_default()
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_vars_in_order_tolerating_whitespace() {
        assert_eq!(extract_vars("{{b}} {{ a }} {{b}}"), vec!["b", "a"]);
        assert!(extract_vars("no vars, not even {single} braces").is_empty());
    }

    #[test]
    fn renders_values_and_blanks_missing() {
        let mut v = HashMap::new();
        v.insert("who".to_string(), "world".to_string());
        assert_eq!(render("hi {{who}}{{gone}}!", &v), "hi world!");
    }
}

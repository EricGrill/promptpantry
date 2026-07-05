use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    dir: Option<PathBuf>,
}

/// Precedence: --dir flag > PROMPT_PANTRY_DIR > ~/.config/prompt-pantry/config.toml `dir` > ~/prompts
pub fn resolve_library_dir(flag: Option<PathBuf>) -> PathBuf {
    resolve_from(
        flag,
        std::env::var("PROMPT_PANTRY_DIR")
            .ok()
            .filter(|s| !s.is_empty()),
        dirs::home_dir(),
    )
}

fn resolve_from(flag: Option<PathBuf>, env: Option<String>, home: Option<PathBuf>) -> PathBuf {
    if let Some(d) = flag {
        return d;
    }
    if let Some(d) = env {
        return PathBuf::from(d);
    }
    if let Some(h) = &home {
        if let Some(d) = read_config_dir(&h.join(".config/prompt-pantry/config.toml")) {
            return d;
        }
    }
    home.map(|h| h.join("prompts"))
        .unwrap_or_else(|| PathBuf::from("prompts"))
}

fn read_config_dir(path: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(path).ok()?;
    toml::from_str::<FileConfig>(&raw).ok()?.dir
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn precedence_flag_env_config_default() {
        let home = TempDir::new().unwrap();
        let cfg = home.path().join(".config/prompt-pantry");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("config.toml"), "dir = \"/from/config\"").unwrap();
        let h = Some(home.path().to_path_buf());
        assert_eq!(
            resolve_from(Some("/flag".into()), Some("/env".into()), h.clone()),
            PathBuf::from("/flag")
        );
        assert_eq!(
            resolve_from(None, Some("/env".into()), h.clone()),
            PathBuf::from("/env")
        );
        assert_eq!(resolve_from(None, None, h), PathBuf::from("/from/config"));
        let empty_home = TempDir::new().unwrap();
        assert_eq!(
            resolve_from(None, None, Some(empty_home.path().to_path_buf())),
            empty_home.path().join("prompts")
        );
    }
}

use std::{env, fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Deserialize, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Ppid,
    Tmux,
    Tty,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Config {
    pub mode: Mode,
    pub unique_top: bool,
}

impl Config {
    pub fn new() -> Self {
        let config_file = Self::get_config_file();
        if let Some(file) = config_file {
            return Self::parse_config_file(&file);
        }

        Self {
            mode: Mode::Tty,
            unique_top: true,
        }
    }

    fn get_config_file() -> Option<String> {
        let config_dir = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = env::var("HOME").expect("Failed to get the HOME environment variable");
            format!("{}/.config", home)
        });
        let possible_suffixes = vec!["yaml", "yml", "json"];
        for suffix in possible_suffixes {
            let path = format!("{}/dirstory.{}", config_dir, suffix);
            if Path::new(&path).exists() {
                return Some(path);
            }
        }

        None
    }

    fn parse_config_file(file: &str) -> Self {
        let content = fs::read_to_string(file).expect("Failed to read the config file");
        if file.ends_with(".yaml") || file.ends_with(".yml") {
            serde_yaml::from_str(&content).unwrap()
        } else if file.ends_with(".json") {
            serde_json::from_str(&content).unwrap()
        } else {
            panic!("Unsupported config file format");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{Builder, NamedTempFile};
    use test_case::test_case;

    #[test_case("---\nmode: tmux\n", Mode::Tmux ; "tmux mode")]
    #[test_case("---\nmode: tty\n", Mode::Tty ; "normal mode")]
    #[test_case("---\nmode: ppid\n", Mode::Ppid ; "ppid mode")]
    fn test_parse_config_file(content: &str, expected: Mode) {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let config = Config::parse_config_file(&path);
        assert_eq!(config.mode, expected);
    }

    #[test_case(".yaml", "---\nmode: tmux\n" ; "yaml suffix")]
    #[test_case(".yml", "---\nmode: tmux\n" ; "yml suffix")]
    #[test_case(".json", r#"{"mode": "tmux"}"# ; "json suffix")]
    fn test_parse_config_file_suffixes(suffix: &str, content: &str) {
        let mut file = Builder::new().suffix(suffix).tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let config = Config::parse_config_file(&path);
        assert_eq!(config.mode, Mode::Tmux);
    }

    #[test]
    fn test_parse_config_file_unsupported_format() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();
        let content = "mode: tmux";
        file.write_all(content.as_bytes()).unwrap();

        let result = std::panic::catch_unwind(|| Config::parse_config_file(&path));
        assert!(result.is_err());
    }
}

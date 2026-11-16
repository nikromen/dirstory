//! Configuration loading with defaults and contextual, recoverable errors.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Default, PartialEq, Deserialize, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Pid,
    Tmux,
    #[default]
    Tty,
}

/// User settings; omitted fields use defaults and unknown fields are errors.
#[derive(Default, Deserialize, Debug, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub mode: Mode,
}

impl Config {
    /// Load the highest-priority existing configuration, or use defaults.
    pub fn new() -> io::Result<Self> {
        let dir = Self::config_dir()?;
        for suffix in ["yaml", "yml", "json"] {
            let path = dir.join(format!("dirstory.{suffix}"));
            match fs::symlink_metadata(&path) {
                Ok(_) => return Self::parse_config_file(&path),
                Err(error) => {
                    if error.kind() != io::ErrorKind::NotFound {
                        return Err(Self::context(&path, error.kind(), error));
                    }
                }
            }
        }
        Ok(Self::default())
    }

    /// Resolve the user's config directory without requiring UTF-8 paths.
    fn config_dir() -> io::Result<PathBuf> {
        if let Some(dir) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return Ok(dir.into());
        }
        env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(|home| PathBuf::from(home).join(".config"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Cannot locate configuration: set XDG_CONFIG_HOME or HOME",
                )
            })
    }

    /// Read one selected file; malformed contents never silently use defaults.
    fn parse_config_file(path: &Path) -> io::Result<Self> {
        let content =
            fs::read_to_string(path).map_err(|error| Self::context(path, error.kind(), error))?;
        if content.trim().is_empty() {
            return Ok(Self::default());
        }
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml" | "yml") => serde_yaml::from_str(&content)
                .map_err(|error| Self::context(path, io::ErrorKind::InvalidData, error)),
            Some("json") => serde_json::from_str(&content)
                .map_err(|error| Self::context(path, io::ErrorKind::InvalidData, error)),
            _ => Err(Self::context(
                path,
                io::ErrorKind::InvalidData,
                "Unsupported configuration format",
            )),
        }
    }

    fn context(path: &Path, kind: io::ErrorKind, error: impl std::fmt::Display) -> io::Error {
        io::Error::new(kind, format!("Configuration {}: {error}", path.display()))
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
    #[test_case("---\nmode: pid\n", Mode::Pid ; "pid mode")]
    fn test_parse_config_file(content: &str, expected: Mode) {
        let mut file = Builder::new().suffix(".yaml").tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let config = Config::parse_config_file(Path::new(&path)).unwrap();
        assert_eq!(config.mode, expected);
    }

    #[test_case(".yaml", "---\nmode: tmux\n" ; "yaml suffix")]
    #[test_case(".yml", "---\nmode: tmux\n" ; "yml suffix")]
    #[test_case(".json", r#"{"mode": "tmux"}"# ; "json suffix")]
    fn test_parse_config_file_suffixes(suffix: &str, content: &str) {
        let mut file = Builder::new().suffix(suffix).tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let path = file.path().to_str().unwrap().to_string();
        let config = Config::parse_config_file(Path::new(&path)).unwrap();
        assert_eq!(config.mode, Mode::Tmux);
    }

    #[test]
    fn test_parse_config_file_unsupported_format() {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();
        let content = "mode: tmux";
        file.write_all(content.as_bytes()).unwrap();

        let result = Config::parse_config_file(Path::new(&path));
        assert!(result.is_err());
    }

    #[test]
    fn omitted_settings_use_tty_in_every_format() {
        for suffix in [".yaml", ".yml", ".json"] {
            for content in ["", " \n\t ", "{}"] {
                let mut file = Builder::new().suffix(suffix).tempfile().unwrap();
                file.write_all(content.as_bytes()).unwrap();
                assert_eq!(
                    Config::parse_config_file(file.path()).unwrap().mode,
                    Mode::Tty
                );
            }
        }
    }

    #[test]
    fn invalid_settings_are_rejected_in_every_format() {
        for (suffix, contents) in [
            (
                ".yaml",
                vec![
                    "mode: invalid",
                    "mode: 42",
                    "unique_top: true",
                    "bad: 1",
                    "[broken",
                ],
            ),
            (
                ".yml",
                vec![
                    "mode: invalid",
                    "mode: 42",
                    "unique_top: true",
                    "bad: 1",
                    "[broken",
                ],
            ),
            (
                ".json",
                vec![
                    r#"{"mode":"invalid"}"#,
                    r#"{"mode":42}"#,
                    r#"{"unique_top":true}"#,
                    r#"{"bad":1}"#,
                    "{broken",
                ],
            ),
        ] {
            for content in contents {
                let mut file = Builder::new().suffix(suffix).tempfile().unwrap();
                file.write_all(content.as_bytes()).unwrap();
                let error = Config::parse_config_file(file.path()).unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                assert!(error
                    .to_string()
                    .contains(&file.path().display().to_string()));
            }
        }
    }
}

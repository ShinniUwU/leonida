//! Where the user's files live.
//!
//! Resolution order for each value: environment variable, then the config
//! file (`LEONIDA_CONFIG`, else `./leonida.toml`), then the default.
//! See `leonida.example.toml` at the repository root.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::graphics::{self, Adapter, Environment, GraphicsSettings, Plan};
use crate::policy::{ensure_outside_repo, is_within};

#[derive(Debug, Default, Clone)]
pub struct Config {
    /// The user's dump of the title.
    pub game_dir: Option<PathBuf>,
    /// Root for all derived data. Always outside the repository.
    pub cache_dir: PathBuf,
    /// What titles write and must keep (persistent local storage). Never
    /// inside the cache, which may be deleted at any time.
    pub data_dir: PathBuf,
    /// Other titles used to exercise platform-level code.
    pub test_titles: Vec<PathBuf>,
    /// The config file that was read, if any.
    pub source: Option<PathBuf>,
    /// Graphics overrides applied before the host device or title starts.
    pub graphics: GraphicsSettings,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    paths: FilePaths,
    #[serde(default)]
    graphics: GraphicsSettings,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePaths {
    game_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    #[serde(default)]
    test_titles: Vec<PathBuf>,
}

impl Config {
    /// Loads configuration from the environment and the config file.
    pub fn load() -> Result<Self> {
        let explicit = std::env::var_os("LEONIDA_CONFIG").map(PathBuf::from);
        let path = file_path();
        let file = if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            Some((path, text))
        } else if let Some(p) = explicit {
            anyhow::bail!(
                "LEONIDA_CONFIG points at {}, which does not exist",
                p.display()
            );
        } else {
            None
        };
        let (source, parsed) = match file {
            Some((path, text)) => {
                let parsed: FileConfig =
                    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                (Some(path), parsed)
            }
            None => (None, FileConfig::default()),
        };
        parsed.graphics.validate()?;
        let mut config = Self::resolve(parsed.paths, source, |k| std::env::var_os(k))?;
        config.graphics = parsed.graphics;
        Ok(config)
    }

    fn resolve(
        file: FilePaths,
        source: Option<PathBuf>,
        env: impl Fn(&str) -> Option<std::ffi::OsString>,
    ) -> Result<Self> {
        let env_path = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let test_titles = match env("LEONIDA_TEST_TITLES") {
            Some(v) if !v.is_empty() => std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .collect(),
            _ => file.test_titles,
        };
        let cache_dir = env_path("LEONIDA_CACHE_DIR")
            .or(file.cache_dir)
            .map_or_else(|| default_dir("cache"), Ok)?;
        let data_dir = env_path("LEONIDA_DATA_DIR")
            .or(file.data_dir)
            .map_or_else(|| default_dir("data"), Ok)?;
        if is_within(&data_dir, &cache_dir)? {
            anyhow::bail!(
                "data_dir ({}) must not be the cache_dir ({}) or inside it: the cache may be \
                 deleted at any time",
                data_dir.display(),
                cache_dir.display()
            );
        }
        Ok(Config {
            game_dir: env_path("LEONIDA_GAME_DIR").or(file.game_dir),
            cache_dir,
            data_dir,
            test_titles,
            source,
            graphics: GraphicsSettings::default(),
        })
    }

    /// Resolves graphics settings without changing the process environment.
    pub fn graphics_plan(&self, gpu: Option<Adapter>, env: &Environment) -> Result<Plan> {
        graphics::load(&self.graphics, self.source.as_deref(), gpu, env)
    }

    /// The game directory, or a helpful error if it is not configured.
    pub fn require_game_dir(&self) -> Result<&Path> {
        self.game_dir.as_deref().context(
            "no game directory configured: run `leonida run` in a terminal to be asked for it, \
             or set LEONIDA_GAME_DIR or [paths].game_dir in leonida.toml",
        )
    }

    /// Writes `game_dir` into `[paths]` of the config file, creating the file
    /// if there is none; its other lines and comments are kept. Returns the
    /// file's path.
    pub fn save_game_dir(game_dir: &Path) -> Result<PathBuf> {
        let path = file_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let new = with_game_dir(&text, game_dir)?;
        let parsed: FileConfig =
            toml::from_str(&new).with_context(|| format!("updating {}", path.display()))?;
        parsed.graphics.validate()?;
        anyhow::ensure!(
            parsed.paths.game_dir.as_deref() == Some(game_dir),
            "updating {}: the game folder did not read back",
            path.display()
        );
        std::fs::write(&path, new).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    /// Returns `<cache>/<sub>`, creating it, after checking the asset policy.
    pub fn cache_subdir(&self, sub: &str) -> Result<PathBuf> {
        subdir(&self.cache_dir, sub)
    }

    /// Returns `<data>/<sub>`, creating it, after checking the asset policy.
    pub fn data_subdir(&self, sub: &str) -> Result<PathBuf> {
        subdir(&self.data_dir, sub)
    }
}

fn subdir(root: &Path, sub: &str) -> Result<PathBuf> {
    let dir = root.join(sub);
    ensure_outside_repo(&dir)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

/// `%LOCALAPPDATA%/leonida/<name>` (elsewhere the XDG or home cache).
fn default_dir(name: &str) -> Result<PathBuf> {
    Ok(user_dir()?.join(name))
}

/// `%LOCALAPPDATA%/leonida` (elsewhere the XDG or home cache), whatever
/// `LEONIDA_DATA_DIR` or leonida.toml say.
pub fn user_dir() -> Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .context("cannot determine a user directory; set LEONIDA_CACHE_DIR and LEONIDA_DATA_DIR")?;
    Ok(base.join("leonida"))
}

/// The config file: `LEONIDA_CONFIG`, else `./leonida.toml`.
fn file_path() -> PathBuf {
    std::env::var_os("LEONIDA_CONFIG").map_or_else(|| PathBuf::from("leonida.toml"), PathBuf::from)
}

/// `text` with `game_dir` set in its `[paths]` table: a `game_dir` line
/// there is replaced, else a commented-out one, else the key goes under the
/// table's header, which is added if missing.
fn with_game_dir(text: &str, game_dir: &Path) -> Result<String> {
    let value = game_dir
        .to_str()
        .with_context(|| format!("{} is not valid UTF-8", game_dir.display()))?;
    let line = format!("game_dir = {}", toml::Value::String(value.to_owned()));
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let key = |l: &str| {
        l.strip_prefix("game_dir")
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    };
    match lines.iter().position(|l| l.trim() == "[paths]") {
        Some(header) => {
            let start = header + 1;
            let end = lines[start..]
                .iter()
                .position(|l| l.trim_start().starts_with('['))
                .map_or(lines.len(), |i| start + i);
            let section = &lines[start..end];
            let at = section.iter().position(|l| key(l.trim_start())).or_else(|| {
                section.iter().position(|l| {
                    l.trim_start()
                        .strip_prefix('#')
                        .is_some_and(|c| key(c.trim_start()))
                })
            });
            match at {
                Some(i) => lines[start + i] = line,
                None => lines.insert(start, line),
            }
        }
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[paths]".to_owned());
            lines.push(line);
        }
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn environment_overrides_file() {
        let file = FilePaths {
            game_dir: Some("from-file".into()),
            cache_dir: Some("cache-from-file".into()),
            ..Default::default()
        };
        let env = |k: &str| (k == "LEONIDA_GAME_DIR").then(|| OsString::from("from-env"));
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.game_dir.unwrap(), PathBuf::from("from-env"));
        assert_eq!(cfg.cache_dir, PathBuf::from("cache-from-file"));
    }

    #[test]
    fn empty_env_values_are_ignored() {
        let file = FilePaths {
            game_dir: Some("from-file".into()),
            ..Default::default()
        };
        let env = |k: &str| (k == "LEONIDA_GAME_DIR").then(OsString::new);
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.game_dir.unwrap(), PathBuf::from("from-file"));
    }

    #[test]
    fn empty_test_titles_env_is_ignored() {
        let file = FilePaths {
            test_titles: vec!["from-file".into()],
            ..Default::default()
        };
        let env = |k: &str| (k == "LEONIDA_TEST_TITLES").then(OsString::new);
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(cfg.test_titles, vec![PathBuf::from("from-file")]);
    }

    #[test]
    fn test_titles_env_splits_paths() {
        let value = std::env::join_paths(["first", "second"]).unwrap();
        let file = FilePaths::default();
        let env = |k: &str| (k == "LEONIDA_TEST_TITLES").then(|| value.clone());
        let cfg = Config::resolve(file, None, env).unwrap();
        assert_eq!(
            cfg.test_titles,
            vec![PathBuf::from("first"), PathBuf::from("second")]
        );
    }

    #[test]
    fn example_config_parses() {
        let text = include_str!("../../../leonida.example.toml");
        let parsed: FileConfig = toml::from_str(text).unwrap();
        assert!(parsed.paths.game_dir.is_none());
    }

    fn game_dir_of(text: &str) -> Option<PathBuf> {
        toml::from_str::<FileConfig>(text).unwrap().paths.game_dir
    }

    #[test]
    fn game_dir_fills_the_example() {
        let text = include_str!("../../../leonida.example.toml");
        let dir = Path::new(r"C:\Games\The Title");
        let new = with_game_dir(text, dir).unwrap();
        assert_eq!(game_dir_of(&new).as_deref(), Some(dir));
        assert_eq!(new.lines().count(), text.lines().count());
        assert!(new.contains("# Derived data."));
    }

    #[test]
    fn game_dir_replaces_a_set_one() {
        let text = "[paths]\ngame_dir = 'old'\ntest_titles = []\n";
        let new = with_game_dir(text, Path::new("new")).unwrap();
        assert_eq!(new, "[paths]\ngame_dir = \"new\"\ntest_titles = []\n");
    }

    #[test]
    fn game_dir_starts_an_empty_file() {
        let new = with_game_dir("", Path::new("D:/game")).unwrap();
        assert_eq!(game_dir_of(&new).unwrap(), PathBuf::from("D:/game"));
    }

    #[test]
    fn game_dir_keeps_crlf() {
        let text = "[paths]\r\n# game_dir = 'x'\r\n";
        let new = with_game_dir(text, Path::new("y")).unwrap();
        assert_eq!(new, "[paths]\r\ngame_dir = \"y\"\r\n");
    }

    #[test]
    fn game_dir_save_preserves_graphics_and_comments() {
        let graphics = "[graphics]\n# Keep the user's choice.\nprofiles = ['none']\nqueue2 = 2\n";
        for text in [
            format!("[paths]\ngame_dir = 'old'\n\n{graphics}"),
            format!("{graphics}\n[paths]\ngame_dir = 'old'\n"),
            graphics.to_owned(),
        ] {
            let new = with_game_dir(&text, Path::new("new")).unwrap();
            assert!(new.contains(graphics));
            let parsed: FileConfig = toml::from_str(&new).unwrap();
            assert_eq!(parsed.paths.game_dir, Some(PathBuf::from("new")));
            assert_eq!(parsed.graphics.queue2, Some(2));
            assert_eq!(parsed.graphics.profiles, Some(vec!["none".into()]));
            parsed.graphics.validate().unwrap();
        }
    }

    #[test]
    fn cache_inside_repo_is_refused() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("cache");
        let cfg = Config {
            cache_dir: inside,
            ..Default::default()
        };
        assert!(cfg.cache_subdir("x").is_err());
    }

    #[test]
    fn data_inside_repo_is_refused() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let cfg = Config {
            data_dir: inside,
            ..Default::default()
        };
        assert!(cfg.data_subdir("x").is_err());
    }

    fn resolve_dirs(cache: &Path, data: &Path) -> Result<Config> {
        let file = FilePaths {
            cache_dir: Some(cache.to_path_buf()),
            data_dir: Some(data.to_path_buf()),
            ..Default::default()
        };
        Config::resolve(file, None, |_| None)
    }

    #[test]
    fn data_dir_equal_to_cache_dir_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_dirs(tmp.path(), tmp.path()).is_err());
    }

    #[test]
    fn data_dir_inside_cache_dir_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        assert!(resolve_dirs(&cache, &cache.join("data")).is_err());
        assert!(resolve_dirs(&cache, &cache.join("a/../data")).is_err());
    }

    #[test]
    fn sibling_with_similar_prefix_and_cache_inside_data_are_allowed() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        assert!(resolve_dirs(&cache, &tmp.path().join("cache-old")).is_ok());
        let data = tmp.path().join("data");
        assert!(resolve_dirs(&data.join("cache"), &data).is_ok());
    }
}

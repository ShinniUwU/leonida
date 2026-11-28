//! The settings file the game and the launcher share:
//! `%LOCALAPPDATA%\leonida\settings\switches.json` ([`path`]), outside any
//! repository, holding `{"version":1,"switches":{"LEONIDA_X":"1"}}`.
//!
//! The F1 menu writes the live switches a player changes there
//! ([`crate::switches::set`], at most a second and a half after the last
//! change and when the menu closes or the game ends); the launcher writes the
//! others. At start the file's values come under an environment variable set
//! by hand and over `leonida.toml` and the `gpu-profiles.json` profiles
//! ([`crate::graphics`]). So a player who starts without switches, turns some
//! on in the game and closes it finds them on at the next start, whichever way
//! the game was started.
//!
//! Only switches of [`crate::switches::SWITCHES`] are taken from the file, and
//! a switch read at start only with one of its listed values. A
//! file that cannot be read as one is left out with a line saying why, never
//! an error: a damaged file must not stop the game. `LEONIDA_SAVED_SETTINGS=0`
//! (or `LEONIDA_GRAPHICS_OFF=1`) leaves the file out, as an unattended
//! measuring run wants. Writes go to a temporary file first, then replace the
//! old one, so a reader never sees half a file; the read and write of a save
//! hold a lock the launcher takes too ([`lock`]), so a change one of them
//! made in between is not written over.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::graphics::Environment;

/// The file's format, its `version` field.
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    #[serde(default)]
    switches: BTreeMap<String, String>,
}

/// `%LOCALAPPDATA%\leonida\settings\switches.json`, after checking it is
/// outside the repository.
pub fn path() -> Result<PathBuf> {
    let path = crate::config::user_dir()?
        .join("settings")
        .join("switches.json");
    crate::ensure_outside_repo(&path)?;
    Ok(path)
}

/// Whether the file is read at start: not with `LEONIDA_SAVED_SETTINGS=0` or
/// `LEONIDA_GRAPHICS_OFF=1` in `env`.
pub fn wanted(env: &Environment) -> bool {
    let set = |name: &str, value: &str| env.get(name).is_some_and(|v| v.trim() == value);
    !set("LEONIDA_SAVED_SETTINGS", "0") && !set("LEONIDA_GRAPHICS_OFF", "1")
}

/// Live switches the start reads too, refusing a value it does not know (leonida-gpu
/// streamline.rs `Asked::from_env` stops on a DLSS model not listed): the file's value must be
/// one of the switch's values or labels, as for a restart switch.
const READ_AT_START: &[&str] = &["LEONIDA_DLSS_PRESET"];

/// The switches the file's text sets, and a line for each entry left out.
pub fn parse(text: &str) -> Result<(Environment, Vec<String>)> {
    let file: File = serde_json::from_str(text).context("not a settings file")?;
    ensure!(
        file.version == VERSION,
        "version {} (this build reads version {VERSION})",
        file.version
    );
    let mut switches = Environment::new();
    let mut notes = Vec::new();
    for (name, value) in file.switches {
        let Some(switch) = crate::switches::find(&name) else {
            notes.push(format!(
                "{name} is not a switch the settings file may set: left out"
            ));
            continue;
        };
        // A restart switch's reader may refuse a value it does not know and stop the start
        // (LEONIDA_DLSS does), which the F1 menu could not undo. Empty leaves it to profiles.
        let known = if !switch.live {
            switch.values.iter().any(|c| c.value == value)
        } else if READ_AT_START.contains(&switch.name) {
            let value = value.trim();
            switch
                .values
                .iter()
                .any(|c| c.value == value || c.label.eq_ignore_ascii_case(value))
        } else {
            true
        };
        if !value.trim().is_empty() && !known {
            notes.push(format!("{name}={value} is not one of its values: left out"));
        } else if value.contains('\0') {
            notes.push(format!("{name} contains a NUL character: left out"));
        } else {
            switches.insert(name, value);
        }
    }
    Ok((switches, notes))
}

/// What [`load_from`] found: the switches, and lines to log about the file.
#[derive(Debug, Default)]
pub struct Loaded {
    pub switches: Environment,
    pub notes: Vec<String>,
}

/// Reads the file at `path`. None there: nothing, said nothing. Damaged:
/// nothing, and a line saying so.
pub fn load_from(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::default(),
        Err(e) => return damaged(path, &e.to_string()),
    };
    match parse(&text) {
        Ok((switches, notes)) => Loaded {
            switches,
            notes: notes
                .into_iter()
                .map(|n| format!("{}: {n}", path.display()))
                .collect(),
        },
        Err(e) => damaged(path, &format!("{e:#}")),
    }
}

fn damaged(path: &Path, why: &str) -> Loaded {
    Loaded {
        switches: Environment::new(),
        notes: vec![format!(
            "{}: {why}; its settings are left out",
            path.display()
        )],
    }
}

/// [`load_from`] the file at [`path`].
pub fn load() -> Loaded {
    match path() {
        Ok(path) => load_from(&path),
        Err(e) => Loaded {
            switches: Environment::new(),
            notes: vec![format!("no settings file: {e:#}")],
        },
    }
}

/// `saved` with `changes` over it.
pub fn merge(saved: &Environment, changes: &BTreeMap<String, String>) -> Environment {
    let mut merged = saved.clone();
    merged.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
    merged
}

/// The file's text for `switches`.
pub fn render(switches: &Environment) -> String {
    let file = File {
        version: VERSION,
        switches: switches.clone(),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("strings always serialize");
    text.push('\n');
    text
}

/// Writes `text` to `path` through a temporary file beside it, which then
/// replaces the old one (`std::fs::rename` replaces an existing file).
/// Replacing can fail for a moment while another program has the file open,
/// so it is tried a few times.
pub fn write_atomically(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().context("the settings file has no folder")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = path.file_name().context("the settings file has no name")?;
    let temp = dir.join(format!(
        "{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
    let mut tries = 0;
    loop {
        match std::fs::rename(&temp, path) {
            Ok(()) => return Ok(()),
            Err(_) if tries < 5 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                return Err(e).with_context(|| format!("replacing {}", path.display()));
            }
        }
    }
}

/// Holds the lock on the settings file at `path` while it lives: the file
/// `switches.json.lock` beside it, open for this handle alone and gone when
/// it closes (also when the process ends). The launcher takes the same lock
/// around its read and write of the file, so neither writes over a change the
/// other made between its read and its write. Waits up to two seconds.
pub fn lock(path: &Path) -> Result<std::fs::File> {
    let lock = path.with_extension("json.lock");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match open_lock(&lock) {
            Ok(file) => return Ok(file),
            Err(_) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => {
                return Err(e).with_context(|| format!("locking {}", lock.display()));
            }
        }
    }
}

#[cfg(windows)]
fn open_lock(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    // Needed for FILE_FLAG_DELETE_ON_CLOSE.
    const DELETE: u32 = 0x0001_0000;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .access_mode(GENERIC_WRITE | DELETE)
        .share_mode(0)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(path)
}

#[cfg(not(windows))]
fn open_lock(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// Moves a file that cannot be read as a settings file to `switches.json.bad`
/// (or `.bad2` and on, so an older one stays), as the launcher's Start fresh
/// does. The name it now has.
fn put_aside(path: &Path) -> Result<PathBuf> {
    for n in 1..100 {
        let suffix = if n == 1 { String::new() } else { n.to_string() };
        let bad = path.with_extension(format!("json.bad{suffix}"));
        if !bad.exists() {
            std::fs::rename(path, &bad)
                .with_context(|| format!("moving {} aside", path.display()))?;
            return Ok(bad);
        }
    }
    anyhow::bail!("{}: too many .bad files beside it", path.display())
}

/// Writes `changes` over what the file at `path` holds, under [`lock`]. Every
/// entry of the file is kept, also those this build does not list (another
/// build or the launcher may). A file that cannot be read is not written
/// over; one that is no settings file of this version is moved aside first
/// ([`put_aside`]), and the changes start a new one.
pub fn save_changes_to(path: &Path, changes: &BTreeMap<String, String>) -> Result<()> {
    let dir = path.parent().context("the settings file has no folder")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let _lock = lock(path)?;
    let saved = match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<File>(&text) {
            Ok(file) if file.version == VERSION => file.switches,
            _ => {
                let bad = put_aside(path)?;
                eprintln!(
                    "settings file: {} could not be read as one; kept as {}",
                    path.display(),
                    bad.display()
                );
                Environment::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Environment::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    write_atomically(path, &render(&merge(&saved, changes)))
}

/// [`save_changes_to`] the file at [`path`]; its path.
pub fn save_changes(changes: &BTreeMap<String, String>) -> Result<PathBuf> {
    let path = path()?;
    save_changes_to(&path, changes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Environment {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_file_parses_to_its_listed_switches() {
        let text = r#"{"version":1,"switches":{"LEONIDA_DLSS":"quality",
            "LEONIDA_KBM_GTA":"1","LEONIDA_CACHE_DIR":"C:/x","LEONIDA_NOPE":"1"}}"#;
        let (switches, notes) = parse(text).unwrap();
        assert_eq!(
            switches,
            env(&[("LEONIDA_DLSS", "quality"), ("LEONIDA_KBM_GTA", "1")])
        );
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes.iter().any(|n| n.starts_with("LEONIDA_CACHE_DIR")));
        assert_eq!(parse(r#"{"version":1}"#).unwrap().0, Environment::new());
    }

    #[test]
    fn a_restart_switch_keeps_only_its_listed_values() {
        let text = r#"{"version":1,"switches":{"LEONIDA_DLSS":"on",
            "LEONIDA_FRAMEGEN_HUDLESS":"","LEONIDA_MOUSE_SENS":"1.5"}}"#;
        let (switches, notes) = parse(text).unwrap();
        assert_eq!(
            switches,
            env(&[("LEONIDA_FRAMEGEN_HUDLESS", ""), ("LEONIDA_MOUSE_SENS", "1.5")])
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("LEONIDA_DLSS=on"));
    }

    #[test]
    fn a_dlss_model_the_start_would_refuse_is_left_out() {
        let text = r#"{"version":1,"switches":{"LEONIDA_DLSS_PRESET":"7",
            "LEONIDA_FRAME_INTERVAL_US":"25000"}}"#;
        let (switches, notes) = parse(text).unwrap();
        assert_eq!(switches, env(&[("LEONIDA_FRAME_INTERVAL_US", "25000")]));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("LEONIDA_DLSS_PRESET=7"));
        for kept in ["11", "k", " K ", "default", ""] {
            let text = format!(r#"{{"version":1,"switches":{{"LEONIDA_DLSS_PRESET":"{kept}"}}}}"#);
            assert_eq!(parse(&text).unwrap().1, Vec::<String>::new(), "{kept:?}");
        }
    }

    #[test]
    fn other_versions_and_shapes_are_refused() {
        for text in [
            r#"{"version":2,"switches":{}}"#,
            r#"{"switches":{}}"#,
            r#"{"version":1,"switches":{"LEONIDA_DLSS":1}}"#,
            r#"[1,2]"#,
            "{\"version\":1,",
            "",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn a_damaged_file_is_left_out_with_a_line_and_a_missing_one_quietly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        let missing = load_from(&path);
        assert!(missing.switches.is_empty() && missing.notes.is_empty());
        std::fs::write(&path, "{\"version\":1,\"switches\":{\"LEONIDA_DLSS\"").unwrap();
        let damaged = load_from(&path);
        assert!(damaged.switches.is_empty());
        assert_eq!(damaged.notes.len(), 1);
        assert!(damaged.notes[0].contains("left out"), "{:?}", damaged.notes);
        // Saving puts it aside and starts a new one with the new values.
        let changes = BTreeMap::from([("LEONIDA_KBM_GTA".to_owned(), "1".to_owned())]);
        save_changes_to(&path, &changes).unwrap();
        assert_eq!(load_from(&path).switches, env(&[("LEONIDA_KBM_GTA", "1")]));
        let bad = dir.path().join("switches.json.bad");
        assert!(std::fs::read_to_string(bad).unwrap().contains("LEONIDA_DLSS"));
    }

    #[test]
    fn saving_keeps_entries_this_build_does_not_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        let other = env(&[("LEONIDA_FROM_ANOTHER_BUILD", "1")]);
        write_atomically(&path, &render(&other)).unwrap();
        let changes = BTreeMap::from([("LEONIDA_KBM_GTA".to_owned(), "1".to_owned())]);
        save_changes_to(&path, &changes).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("LEONIDA_FROM_ANOTHER_BUILD"), "{text}");
        assert!(text.contains("LEONIDA_KBM_GTA"), "{text}");
    }

    #[cfg(windows)]
    #[test]
    fn the_lock_is_one_holders_and_goes_when_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switches.json");
        let held = lock(&path).unwrap();
        assert!(dir.path().join("switches.json.lock").exists());
        assert!(open_lock(&dir.path().join("switches.json.lock")).is_err());
        drop(held);
        assert!(!dir.path().join("switches.json.lock").exists());
        drop(lock(&path).unwrap());
    }

    #[test]
    fn saving_merges_over_what_the_launcher_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings").join("switches.json");
        let launcher = env(&[("LEONIDA_DLSS", "quality"), ("LEONIDA_KBM_GTA", "0")]);
        write_atomically(&path, &render(&launcher)).unwrap();
        let changes = BTreeMap::from([
            ("LEONIDA_KBM_GTA".to_owned(), "1".to_owned()),
            ("LEONIDA_MOUSE_SENS".to_owned(), "1.5".to_owned()),
        ]);
        save_changes_to(&path, &changes).unwrap();
        let loaded = load_from(&path);
        assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
        assert_eq!(
            loaded.switches,
            env(&[
                ("LEONIDA_DLSS", "quality"),
                ("LEONIDA_KBM_GTA", "1"),
                ("LEONIDA_MOUSE_SENS", "1.5"),
            ])
        );
        // No temporary file is left beside it.
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["switches.json"]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"version\": 1,"), "{text}");
    }

    #[test]
    fn the_file_is_skipped_for_measuring_runs() {
        assert!(wanted(&env(&[])));
        assert!(wanted(&env(&[("LEONIDA_SAVED_SETTINGS", "1")])));
        assert!(!wanted(&env(&[("LEONIDA_SAVED_SETTINGS", "0")])));
        assert!(!wanted(&env(&[("LEONIDA_GRAPHICS_OFF", "1")])));
    }

    #[test]
    fn the_file_lives_outside_the_repository() {
        if let Ok(path) = path() {
            assert!(
                path.ends_with("leonida/settings/switches.json"),
                "{}",
                path.display()
            );
        }
    }
}

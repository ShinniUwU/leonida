//! The switches a player may touch, in one list ([`SWITCHES`]): for each, its
//! `LEONIDA_*` name, a short label, one line of help, the group it is shown
//! in, the values it takes with their labels, what the code does without it
//! (`default`), and whether a change applies while the game runs (`live`).
//! The F1 settings menu shows the live ones only; the launcher reads the
//! whole list from `leonida config --switches-json` and shows the restart
//! ones as needing a restart, so neither hard-codes it. In the game a
//! restart switch is chosen for the next start with [`set_for_next_start`]
//! (the F1 menu no longer does).
//!
//! A switch is live only when changing it mid-game is safe: it is read every
//! frame or at each use, never decides device, heap, queue or swap-chain
//! setup, and never feeds the shader cache key (switches the shader crate's
//! `build.rs` collects are always restart ones). Live switches are read
//! through [`live_number`] and [`live_flag`]: from the environment at first
//! (where the settings file and the profiles were applied at start, see
//! [`crate::graphics`]), then as [`set`] changes them. The others stay plain
//! environment reads, once.
//!
//! [`set`] also notes the change for the shared settings file
//! ([`crate::settings`]); [`save_due`] and [`save_now`] write it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;

/// Where a switch is shown: a tab of the F1 menu for live ones, a section of
/// the launcher for the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    Display,
    FrameGeneration,
    Upscaling,
    Picture,
    Controls,
    Performance,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Display,
        Group::FrameGeneration,
        Group::Upscaling,
        Group::Picture,
        Group::Controls,
        Group::Performance,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Display => "DISPLAY",
            Group::FrameGeneration => "FRAME GENERATION",
            Group::Upscaling => "UPSCALING",
            Group::Picture => "PICTURE",
            Group::Controls => "CONTROLS",
            Group::Performance => "PERFORMANCE",
        }
    }
}

/// A value a switch takes, and what it is called in the menu.
#[derive(Debug, Serialize)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Switch {
    pub name: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub group: Group,
    pub values: &'static [Choice],
    /// The value that does what the code does with the switch unset (a
    /// profile may still set another).
    pub default: &'static str,
    pub live: bool,
}

const fn c(value: &'static str, label: &'static str) -> Choice {
    Choice { value, label }
}

const OFF_ON: &[Choice] = &[c("0", "OFF"), c("1", "ON")];

#[rustfmt::skip]
const LIST: &[Switch] = &[
    // Live: read at each use, nothing set up from them.
    Switch {
        name: "LEONIDA_FPS_COUNTER",
        label: "FPS ON SCREEN",
        help: "Shows the frame rate in the top left corner of the picture.",
        group: Group::Display,
        values: OFF_ON,
        default: "1",
        live: true,
    },
    Switch {
        name: "LEONIDA_PANEL",
        label: "PERFORMANCE PANEL",
        help: "The frame rate in large type, with frame time, draws and the card's memory. F3 \
               shows and hides it.",
        group: Group::Display,
        values: OFF_ON,
        default: "0",
        live: true,
    },
    Switch {
        name: "LEONIDA_FRAME_INTERVAL_US",
        label: "FRAME-RATE CAP",
        help: "The pace the game is asked to draw at. The game's own is the pace it asks for.",
        group: Group::Display,
        values: &[c("0", "GAME'S OWN"), c("33333", "30 FPS"), c("16667", "60 FPS"),
                  c("11111", "90 FPS"), c("8333", "120 FPS"), c("6944", "144 FPS"),
                  c("1000", "NONE")],
        default: "0",
        live: true,
    },
    Switch {
        name: "LEONIDA_FRAMEGEN_X",
        label: "FRAME GENERATION AMOUNT",
        help: "Frames shown for each one the game draws, when frame generation started with the \
               game. FSR 3 shows 2 at most, DLSS more only on an RTX 50.",
        group: Group::FrameGeneration,
        values: &[c("0", "OFF"), c("2", "2X"), c("3", "3X"), c("4", "4X"), c("5", "5X"),
                  c("6", "6X")],
        default: "2",
        live: true,
    },
    Switch {
        name: "LEONIDA_KBM_GTA",
        label: "GTA V KEYS AND MOUSE",
        help: "Keys laid out like GTA V on PC; F starts driving (W, S, Space), F2 if it got \
               out of step. The mouse needs MOUSE CAPTURE on.",
        group: Group::Controls,
        values: OFF_ON,
        default: "0",
        live: true,
    },
    Switch {
        name: "LEONIDA_MOUSE_SENS",
        label: "MOUSE SENSITIVITY",
        help: "How fast the mouse turns the camera (with MOUSE CAPTURE on).",
        group: Group::Controls,
        values: &[c("0.25", "0.25"), c("0.5", "0.5"), c("0.75", "0.75"), c("1", "1.0"),
                  c("1.25", "1.25"), c("1.5", "1.5"), c("2", "2.0"), c("3", "3.0")],
        default: "1",
        live: true,
    },
    Switch {
        name: "LEONIDA_MOUSE_INVERT_Y",
        label: "INVERT MOUSE Y",
        help: "The mouse turns the camera up and down the other way round.",
        group: Group::Controls,
        values: OFF_ON,
        default: "0",
        live: true,
    },
    Switch {
        name: "LEONIDA_MOUSE_CAPTURE",
        label: "MOUSE CAPTURE",
        help: "Keeps the cursor hidden inside the game's window while it is in front, and \
               gives the game a mouse to turn the camera with. F1 frees the cursor.",
        group: Group::Controls,
        values: OFF_ON,
        default: "0",
        live: true,
    },
    // Restart: read once, or deciding what is set up at start.
    Switch {
        name: "LEONIDA_MENU",
        label: "F1 SETTINGS MENU",
        help: "The settings menu on F1 and the panel on F3 while the game runs. Off: both keys \
               go to the game.",
        group: Group::Display,
        values: OFF_ON,
        default: "1",
        live: false,
    },
    Switch {
        name: "LEONIDA_DLSS",
        label: "DLSS",
        help: "NVIDIA DLSS upscaling on RTX cards. Needs NVIDIA's files \
               (scripts\\get-nvidia-dlss.ps1).",
        group: Group::Upscaling,
        values: &[c("off", "OFF"), c("quality", "QUALITY"), c("balanced", "BALANCED"),
                  c("performance", "PERFORMANCE"), c("dlaa", "DLAA (NATIVE)")],
        default: "off",
        live: false,
    },
    Switch {
        name: "LEONIDA_DLSS_PRESET",
        label: "DLSS MODEL",
        help: "Which DLSS model upscales. The default is NVIDIA's choice.",
        group: Group::Upscaling,
        values: &[c("default", "DEFAULT"), c("e", "E"), c("f", "F"), c("j", "J"), c("k", "K"),
                  c("l", "L"), c("m", "M")],
        default: "default",
        live: false,
    },
    Switch {
        name: "LEONIDA_FRAMEGEN",
        label: "DLSS FRAME GENERATION",
        help: "DLSS Frame Generation on RTX 40 and 50 cards. Needs DLSS on.",
        group: Group::FrameGeneration,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_FSR_FRAMEGEN",
        label: "FSR 3 FRAME GENERATION",
        help: "AMD FSR 3 frame generation where DLSS Frame Generation does not run. Needs DLSS \
               on and AMD's files (scripts\\get-amd-fsr.ps1).",
        group: Group::FrameGeneration,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    // Someone else's mod (github.com/dashdogy/MFG-Unlocker, MIT), which the
    // launcher fetches and loads only with this on (apps/leonida/src/mfg.rs).
    Switch {
        name: "LEONIDA_MFG_UNLOCKER",
        label: "MFG UNLOCKER (MOD)",
        help: "Someone else's mod (MFG Unlocker): DLSS frame generation on RTX 20 and 30. \
               NVIDIA does not support it, and it was not tested with this game. Applies at \
               the next start.",
        group: Group::FrameGeneration,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_FRAMEGEN_HUDLESS",
        label: "PICTURE WITHOUT INTERFACE",
        help: "Gives frame generation the picture without the interface, so text and the map \
               do not smear.",
        group: Group::FrameGeneration,
        values: OFF_ON,
        default: "1",
        live: false,
    },
    Switch {
        name: "LEONIDA_LEVELS_ON_DEMAND",
        label: "TEXTURE LEVELS ON DEMAND",
        help: "Gives texture detail memory only when the game shows it.",
        group: Group::Picture,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_WATER_LIGHT",
        label: "WATER LIGHT",
        help: "The water's tiled light. Off leaves it out where it drew dark blocks.",
        group: Group::Picture,
        values: OFF_ON,
        default: "1",
        live: false,
    },
    Switch {
        name: "LEONIDA_AMBIENT",
        label: "LIGHT ESTIMATE",
        help: "Lights rooms the sun and lamps do not reach, in place of the console's traced \
               light.",
        group: Group::Picture,
        values: &[c("0", "OFF"), c("0.5", "0.5"), c("0.8", "0.8"), c("1", "1.0")],
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_AMBIENT_SPECULAR",
        label: "REFLECTION ESTIMATE",
        help: "Reflections from the light around, in place of the console's traced ones.",
        group: Group::Picture,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_AMBIENT_AIR",
        label: "FOG CAP",
        help: "The most the air may glow, as a multiple of the light around. Off: the game's.",
        group: Group::Picture,
        values: &[c("0", "OFF"), c("1", "1.0"), c("1.5", "1.5"), c("2", "2.0")],
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_EARLY_DEPTH",
        label: "EARLY DEPTH TEST",
        help: "Keeps glass behind a head from drawing over it. Translates the shaders again.",
        group: Group::Picture,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_SKIP_SCENES",
        label: "SKIP CUTSCENES",
        help: "Presses Enter while the picture shows a cutscene.",
        group: Group::Controls,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_ASYNC_SUBMIT",
        label: "SUBMIT ON A THREAD",
        help: "Prepares the card's work on threads of its own. Frame generation needs it or \
               thin lists.",
        group: Group::Performance,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    // VIctory-NVIDIA 5986a64 put these three in its F1 menu for the next
    // start: on an RTX 5080 heads were missing with THIN on.
    Switch {
        name: "LEONIDA_THIN",
        label: "THIN LISTS",
        help: "Records the game's drawing calls on threads of their own. Heads were missing \
               with it on an RTX 5080.",
        group: Group::Performance,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_JOIN_PARTS",
        label: "JOIN PARTS",
        help: "How many parts of the game's drawing go to the card together.",
        group: Group::Performance,
        values: &[c("1", "1 (OFF)"), c("8", "8")],
        default: "1",
        live: false,
    },
    Switch {
        name: "LEONIDA_TEARING",
        label: "TEARING",
        help: "Shows a frame as soon as it is ready, without waiting for the screen. The \
               picture can tear.",
        group: Group::Display,
        values: OFF_ON,
        default: "1",
        live: false,
    },
    Switch {
        name: "LEONIDA_FULLSCREEN",
        label: "FULLSCREEN",
        help: "The game's window covers the whole screen, without a border.",
        group: Group::Display,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    Switch {
        name: "LEONIDA_IME_ENGLISH",
        label: "ENGLISH KEYBOARD",
        help: "Keeps the keyboard on the English layout while the game is in front.",
        group: Group::Controls,
        values: OFF_ON,
        default: "0",
        live: false,
    },
    // AUTO (no value): the profiles' choice, else one worked out from the
    // card's memory budget.
    Switch {
        name: "LEONIDA_TEXTURE_ROOM_PERCENT",
        label: "TEXTURE ROOM",
        help: "How much of the card's memory textures may fill, in percent. Lower leaves room \
               for frame generation.",
        group: Group::Performance,
        values: &[c("", "AUTO"), c("70", "70%"), c("80", "80%"), c("86", "86%"),
                  c("90", "90%"), c("94", "94%")],
        default: "",
        live: false,
    },
    Switch {
        name: "LEONIDA_TEXTURE_STAGE_MB",
        label: "TEXTURE UPLOADS PER FRAME",
        help: "Megabytes of textures sent to the card each frame at most.",
        group: Group::Performance,
        values: &[c("", "AUTO"), c("128", "128 MB"), c("192", "192 MB"),
                  c("256", "256 MB"), c("512", "512 MB"), c("1024", "1024 MB")],
        default: "",
        live: false,
    },
];

/// Every switch a player may touch.
pub static SWITCHES: &[Switch] = LIST;

/// The switch named `name`.
pub fn find(name: &str) -> Option<&'static Switch> {
    index(name).map(|i| &LIST[i])
}

fn index(name: &str) -> Option<usize> {
    LIST.iter().position(|s| s.name == name)
}

/// Not read yet: a NaN that [`number`] never gives (it gives finite values).
const UNREAD: u64 = u64::MAX;

/// The live switches' values as `f64` bits, by place in [`SWITCHES`].
static LIVE: [AtomicU64; LIST.len()] = [const { AtomicU64::new(UNREAD) }; LIST.len()];

/// A switch's value as a number: `1` and `0` for the words on and off (`true`,
/// `yes`, `on`; `false`, `no`, `off`), else the number. None when empty or not
/// a finite number.
pub fn number(value: &str) -> Option<f64> {
    let value = value.trim();
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => return Some(1.0),
        "false" | "no" | "off" => return Some(0.0),
        _ => {}
    }
    value.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A live switch's value now: as [`set`] last made it, else the environment's
/// at the first read, else the switch's default; 0 for a name not in the list
/// or a value that is no number.
pub fn live_number(name: &str) -> f64 {
    let Some(i) = index(name) else {
        return 0.0;
    };
    let bits = LIVE[i].load(Ordering::Relaxed);
    if bits != UNREAD {
        return f64::from_bits(bits);
    }
    let start = std::env::var(name).ok().as_deref().and_then(number);
    let value = start.or_else(|| number(LIST[i].default)).unwrap_or(0.0);
    // A `set` between the load and here wins.
    let _ = LIVE[i].compare_exchange(
        UNREAD,
        value.to_bits(),
        Ordering::Relaxed,
        Ordering::Relaxed,
    );
    f64::from_bits(LIVE[i].load(Ordering::Relaxed))
}

/// [`live_number`] as on (not 0) or off.
pub fn live_flag(name: &str) -> bool {
    live_number(name) != 0.0
}

/// What [`set`] chose this run, and whether some of it is not in the settings
/// file yet (the time of the last change then).
struct Chosen {
    values: BTreeMap<String, String>,
    unsaved_since: Option<Instant>,
}

static CHOSEN: Mutex<Chosen> = Mutex::new(Chosen {
    values: BTreeMap::new(),
    unsaved_since: None,
});

fn chosen() -> std::sync::MutexGuard<'static, Chosen> {
    CHOSEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// A switch's value as text, the way the menu shows it: what [`set`] chose,
/// else the environment's, else the default.
pub fn text(name: &str) -> String {
    if let Some(value) = chosen().values.get(name) {
        return value.clone();
    }
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| find(name).map(|s| s.default.to_owned()))
        .unwrap_or_default()
}

/// Changes a live switch while the game runs and notes the change for the
/// settings file. False, and nothing changed: not a live switch, or `value` is
/// no number.
pub fn set(name: &str, value: &str) -> bool {
    let Some(i) = index(name).filter(|&i| LIST[i].live) else {
        return false;
    };
    let Some(n) = number(value) else {
        return false;
    };
    LIVE[i].store(n.to_bits(), Ordering::Relaxed);
    let mut chosen = chosen();
    chosen.values.insert(name.to_owned(), value.to_owned());
    chosen.unsaved_since = Some(Instant::now());
    FAILED.store(0, Ordering::Relaxed);
    true
}

/// Chooses a restart switch's value for the next start: noted for the settings
/// file only, this run keeps the value it started with. False, and nothing
/// noted: a live switch, not in the list, or `value` is not one of its values.
pub fn set_for_next_start(name: &str, value: &str) -> bool {
    let Some(s) = find(name).filter(|s| !s.live) else {
        return false;
    };
    if !s.values.iter().any(|c| c.value == value) {
        return false;
    }
    let mut chosen = chosen();
    chosen.values.insert(name.to_owned(), value.to_owned());
    chosen.unsaved_since = Some(Instant::now());
    FAILED.store(0, Ordering::Relaxed);
    true
}

/// Writes what [`set`] chose into the settings file once `quiet` has passed
/// since the last change. None: nothing to write, or not yet.
pub fn save_due(quiet: Duration) -> Option<Result<PathBuf>> {
    save(|since| since.elapsed() >= quiet)
}

/// Whether [`save_due`] would write now (the menu asks at every frame),
/// without waiting for a save under way.
pub fn save_is_due(quiet: Duration) -> bool {
    chosen()
        .unsaved_since
        .is_some_and(|since| since.elapsed() >= quiet)
}

/// Writes what [`set`] chose into the settings file now, if some of it is not
/// there yet (the menu closes, or the game ends).
pub fn save_now() -> Option<Result<PathBuf>> {
    save(|_| true)
}

/// Saves one at a time, in the order their values were taken.
static SAVING: Mutex<()> = Mutex::new(());
/// Saves that failed in a row since the last change.
static FAILED: AtomicU64 = AtomicU64::new(0);
/// A save that fails is tried again this many times in all, a quiet time
/// apart, so a file that cannot be written is not said at every frame.
const TRIES: u64 = 3;

fn save(due: impl FnOnce(Instant) -> bool) -> Option<Result<PathBuf>> {
    save_with(due, crate::settings::save_changes)
}

/// [`save`], writing through `write` (the settings file's [`crate::settings::save_changes`]).
fn save_with(
    due: impl FnOnce(Instant) -> bool,
    write: impl FnOnce(&BTreeMap<String, String>) -> Result<PathBuf>,
) -> Option<Result<PathBuf>> {
    let _saving = SAVING.lock().unwrap_or_else(|e| e.into_inner());
    // Taken out under the lock and written without it: the menu reads and
    // changes values while the file is written.
    let values = {
        let mut chosen = chosen();
        if !due(chosen.unsaved_since?) {
            return None;
        }
        chosen.unsaved_since = None;
        chosen.values.clone()
    };
    let saved = write(&values);
    if saved.is_ok() {
        FAILED.store(0, Ordering::Relaxed);
    } else if FAILED.fetch_add(1, Ordering::Relaxed) + 1 < TRIES {
        chosen().unsaved_since.get_or_insert_with(Instant::now);
    }
    Some(saved)
}

#[derive(Serialize)]
struct GroupInfo {
    id: Group,
    label: &'static str,
}

#[derive(Serialize)]
struct Registry<'a> {
    version: u32,
    settings_file: Option<String>,
    groups: Vec<GroupInfo>,
    switches: &'a [Switch],
}

/// The list as JSON for the launcher (`leonida config --switches-json`), with
/// the settings file's path when there is one.
pub fn to_json(settings_file: Option<&Path>) -> Result<String> {
    let registry = Registry {
        version: crate::settings::VERSION,
        settings_file: settings_file.map(|p| p.display().to_string()),
        groups: Group::ALL
            .iter()
            .map(|&id| GroupInfo {
                id,
                label: id.label(),
            })
            .collect(),
        switches: LIST,
    };
    serde_json::to_string_pretty(&registry).context("serializing the switch list")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::is_setting_key;

    #[test]
    fn every_switch_is_complete_and_named_once() {
        for (i, s) in LIST.iter().enumerate() {
            assert!(is_setting_key(s.name), "{}", s.name);
            assert_eq!(index(s.name), Some(i), "{} listed twice", s.name);
            assert!(
                !s.label.is_empty() && s.label == s.label.to_ascii_uppercase(),
                "{}",
                s.name
            );
            assert!(
                s.label.len() <= 28,
                "{}: label too long for the menu",
                s.name
            );
            assert!(!s.help.is_empty() && s.help.ends_with('.'), "{}", s.name);
            assert!(s.values.len() >= 2, "{}", s.name);
            assert!(
                s.values.iter().any(|v| v.value == s.default),
                "{}: default not a value",
                s.name
            );
            for text in [s.label, s.help]
                .into_iter()
                .chain(s.values.iter().map(|v| v.label))
            {
                assert!(!text.contains('\u{2014}'), "{}: long dash", s.name);
            }
            if s.live {
                assert!(
                    s.values.iter().all(|v| number(v.value).is_some()),
                    "{}",
                    s.name
                );
            }
        }
    }

    #[test]
    fn only_switches_read_at_each_use_are_live() {
        let live: Vec<&str> = LIST.iter().filter(|s| s.live).map(|s| s.name).collect();
        assert_eq!(
            live,
            [
                "LEONIDA_FPS_COUNTER",
                "LEONIDA_PANEL",
                "LEONIDA_FRAME_INTERVAL_US",
                "LEONIDA_FRAMEGEN_X",
                "LEONIDA_KBM_GTA",
                "LEONIDA_MOUSE_SENS",
                "LEONIDA_MOUSE_INVERT_Y",
                "LEONIDA_MOUSE_CAPTURE",
            ]
        );
        // Part of the shader cache key: never live.
        assert!(!find("LEONIDA_EARLY_DEPTH").unwrap().live);
    }

    #[test]
    fn numbers_take_words_and_refuse_blanks() {
        assert_eq!(number(" 1 "), Some(1.0));
        assert_eq!(number("On"), Some(1.0));
        assert_eq!(number("no"), Some(0.0));
        assert_eq!(number("1.25"), Some(1.25));
        assert_eq!(number(""), None);
        assert_eq!(number("fast"), None);
        assert_eq!(number("NaN"), None);
        assert_eq!(number("inf"), None);
    }

    #[test]
    fn set_changes_only_live_switches_and_is_read_back() {
        assert!(!set("LEONIDA_DLSS", "quality"), "a restart switch");
        assert!(!set("LEONIDA_NOT_LISTED", "1"));
        assert!(!set("LEONIDA_MOUSE_SENS", "fast"));
        assert!(set("LEONIDA_MOUSE_SENS", "1.5"));
        assert_eq!(live_number("LEONIDA_MOUSE_SENS"), 1.5);
        assert_eq!(text("LEONIDA_MOUSE_SENS"), "1.5");
        assert!(set("LEONIDA_MOUSE_SENS", "0.5"));
        assert_eq!(live_number("LEONIDA_MOUSE_SENS"), 0.5);
        assert_eq!(live_number("LEONIDA_NOT_LISTED"), 0.0);
    }

    #[test]
    fn restart_switches_are_chosen_for_the_next_start_only() {
        assert!(!set_for_next_start("LEONIDA_KBM_GTA", "1"), "a live switch");
        assert!(!set_for_next_start("LEONIDA_NOT_LISTED", "1"));
        assert!(!set_for_next_start("LEONIDA_MFG_UNLOCKER", "yes"), "not one of its values");
        assert!(set_for_next_start("LEONIDA_MFG_UNLOCKER", "1"));
        assert_eq!(text("LEONIDA_MFG_UNLOCKER"), "1");
        assert!(chosen().values.get("LEONIDA_MFG_UNLOCKER").is_some_and(|v| v == "1"));
        assert!(!set("LEONIDA_MFG_UNLOCKER", "1"), "never live");
        let mfg = find("LEONIDA_MFG_UNLOCKER").unwrap();
        assert_eq!((mfg.default, mfg.group), ("0", Group::FrameGeneration));
        assert!(mfg.help.contains("NVIDIA does not support it"));
    }

    #[test]
    fn a_menu_change_is_saved_and_resolved_at_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings").join("switches.json");
        let write = |values: &BTreeMap<String, String>| {
            crate::settings::save_changes_to(&path, values).map(|()| path.clone())
        };
        // What the F1 menu does for a row: `set`, then a save once changes
        // are quiet (or the menu closes).
        assert!(set("LEONIDA_FRAME_INTERVAL_US", "16667"));
        let saved = save_with(|_| true, write).expect("a change to save");
        assert_eq!(saved.unwrap(), path);
        // The next start: the file is read and resolved over the profiles.
        let loaded = crate::settings::load_from(&path);
        assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
        let plan = crate::graphics::resolve_with_saved(
            &crate::graphics::GraphicsSettings::default(),
            None,
            &crate::graphics::Environment::new(),
            &loaded.switches,
            None,
        )
        .unwrap();
        assert_eq!(plan.settings["LEONIDA_FRAME_INTERVAL_US"], "16667");
        assert_eq!(plan.sources["LEONIDA_FRAME_INTERVAL_US"], "settings");
    }

    #[test]
    fn the_json_lists_every_switch_with_its_fields() {
        let json = to_json(Some(Path::new("C:/x/switches.json"))).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["settings_file"], "C:/x/switches.json");
        assert_eq!(value["groups"][1]["id"], "frame-generation");
        let switches = value["switches"].as_array().unwrap();
        assert_eq!(switches.len(), LIST.len());
        let fps = &switches[0];
        assert_eq!(fps["name"], "LEONIDA_FPS_COUNTER");
        assert_eq!(fps["group"], "display");
        assert_eq!(fps["live"], true);
        assert_eq!(fps["default"], "1");
        assert_eq!(fps["values"][1]["label"], "ON");
        for key in ["label", "help"] {
            assert!(fps[key].is_string(), "{key}");
        }
    }
}

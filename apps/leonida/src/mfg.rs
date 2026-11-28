//! MFG Unlocker as an opt-in: someone else's mod (github.com/dashdogy/MFG-Unlocker,
//! MIT licence) that unlocks DLSS Frame Generation on GeForce RTX 20 and 30
//! cards, which NVIDIA does not support; it was not tested with this game.
//! `LEONIDA_MFG_UNLOCKER=1` turns it on (F1, FRAME GENERATION, MFG UNLOCKER, or
//! a yes to the question below); unset or 0, nothing of it is fetched or run.
//!
//! - The question, default no: once, at the start of `leonida run`, on an RTX
//!   20 or 30 with DLSS and DLSS Frame Generation asked for, where the switch
//!   was never set (by hand, in the settings file, `leonida.toml` or a
//!   profile) and no mod DLL sits beside leonida.exe already; only with a
//!   terminal to answer in. A yes goes into the settings file, as the F1 menu
//!   would put it; a no is kept (a `declined` file) and not asked again.
//! - On, with DLSS Frame Generation asked for: `scripts\get-mfg-unlocker.ps1`
//!   (carried in the executable) fetches it where it is missing from the
//!   mod's GitHub release into `%LOCALAPPDATA%\leonida\mfg-unlocker`, checked
//!   against the SHA-256 its author published, with its licence. It is then
//!   loaded under its own name before Streamline and the device, after its
//!   SHA-256 is checked again (`leonida_hle::streamline::load_mod`). It writes
//!   its own logs and settings (`RTXMFG-...`) beside the program, which we
//!   cannot move.
//!
//! Ported from trustno1-coder/VIctory-NVIDIA (0cef07e: `older_rtx`,
//! `offer_mfg`, `mfg_dll` and `load_mfg` in `apps/victory/src/play.rs`;
//! 10862a6: `mod_dlls`), GPL-3.0-or-later.

use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use leonida_hle::streamline::{self, Asked};

/// The switch that turns it on.
const SWITCH: &str = "LEONIDA_MFG_UNLOCKER";
const GET: (&str, &str) = (
    "get-mfg-unlocker.ps1",
    include_str!("../../../scripts/get-mfg-unlocker.ps1"),
);
/// The mod's DLL, under its own name: loaded by us, not by Windows in place
/// of one of its own, so nothing is renamed or put beside the game.
const DLL: &str = "MFGUnlocker.dll";
/// The SHA-256 of v1.7.2's DLL its author published, as the script checks it.
const DLL_SHA256: &str = "A9B1C01727FA72763BEF8F604E44EC165CE1C98272DE00719572B766A11CF28F";
/// How the log and the notice name it.
const NAME: &str = "MFG Unlocker (someone else's mod)";

/// `%LOCALAPPDATA%\leonida\mfg-unlocker`.
fn home() -> anyhow::Result<PathBuf> {
    let home = leonida_core::config::user_dir()?.join("mfg-unlocker");
    leonida_core::ensure_outside_repo(&home)?;
    Ok(home)
}

/// At the start of `leonida run`, before Streamline is loaded: asks where it
/// should, then fetches and loads the mod where it is on.
pub(crate) fn start(asked: &Asked) {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(mods_in))
        .unwrap_or_default();
    if !beside.is_empty() {
        println!(
            "mods           beside leonida.exe, loaded by Windows in place of its own: {} (a mod, \
             such as MFG Unlocker)",
            beside.join(", ")
        );
    }
    let framegen = asked.dlss && asked.framegen;
    let never_set = std::env::var_os(SWITCH).is_none_or(|v| v.is_empty());
    if never_set && framegen && beside.is_empty() {
        let card = streamline::adapter_name().unwrap_or_default();
        if older_rtx(&card) && ask(&card) {
            turn_on();
        }
    }
    let on = std::env::var(SWITCH)
        .ok()
        .as_deref()
        .and_then(leonida_core::switches::number)
        .is_some_and(|v| v != 0.0);
    if !on {
        return;
    }
    if !framegen {
        println!(
            "mfg unlocker   on, not loaded: it needs DLSS and LEONIDA_FRAMEGEN=1 (the \
             dlss-framegen profile)"
        );
        return;
    }
    let home = match home() {
        Ok(home) => home,
        Err(e) => {
            println!("mfg unlocker   not loaded: {e:#}");
            return;
        }
    };
    let dll = home.join(DLL);
    if !dll.is_file() {
        println!("mfg unlocker   on and not on this PC yet: downloading it");
        if let Err(e) = crate::offer::run_script(GET, &home) {
            println!("mfg unlocker   not downloaded ({e:#}); the game runs without it");
            return;
        }
    }
    let pin = sha256_from_hex(DLL_SHA256).expect("a valid pinned hash");
    match streamline::load_mod(NAME, &dll, &pin) {
        Ok(()) => println!(
            "mfg unlocker   loaded ({}); Backspace opens its menu. NVIDIA does not support it: \
             if the game fails, turn it off (F1, MFG UNLOCKER)",
            dll.display()
        ),
        Err(why) => println!("mfg unlocker   not loaded: {why}; the game runs without it"),
    }
}

/// A GeForce RTX of the 20 or 30 series ("NVIDIA GeForce RTX 3080", "... RTX
/// 2060 SUPER"): DLSS runs, DLSS Frame Generation only with the mod.
fn older_rtx(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    let Some(at) = name.find("RTX") else { return false };
    let model: String =
        name[at + 3..].trim_start().chars().take_while(char::is_ascii_digit).collect();
    model.len() == 4 && (model.starts_with("20") || model.starts_with("30"))
}

/// What the question says after naming the card.
const QUESTION: &str = "\
and later. MFG Unlocker, someone else's mod (github.com/dashdogy/MFG-Unlocker, MIT
licence, about 5 MB), unlocks it on RTX 20 and 30 cards. NVIDIA does not support that,
and it was not tested with this game. On, it is downloaded now from its GitHub release
(checked against its published SHA-256) and loaded at every start; F1, MFG UNLOCKER
turns it off again. Off, AMD's FSR 3 frame generation (2x) is used where it is set up.";

/// Asks once whether to turn it on. No is the default: it is someone else's
/// mod, NVIDIA does not support what it does, and it was not tested here.
fn ask(card: &str) -> bool {
    let Ok(home) = home() else { return false };
    let declined = home.join("declined");
    if declined.is_file() || !std::io::stdin().is_terminal() {
        return false;
    }
    println!("This card ({card}) runs DLSS, but NVIDIA's DLSS Frame Generation only on RTX 40");
    println!("{QUESTION}");
    print!("Turn MFG Unlocker on? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    if !says_yes(&answer) {
        let _ = std::fs::create_dir_all(&home);
        let _ = std::fs::write(&declined, "asked once; F1, MFG UNLOCKER turns it on\n");
        println!("Left off; not asked again. F1, MFG UNLOCKER turns it on.");
        return false;
    }
    true
}

/// Only "y" or "yes", in any case: an empty answer is no.
fn says_yes(answer: &str) -> bool {
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

/// The yes: into the settings file, as the F1 menu would put it, and on for
/// this start.
fn turn_on() {
    let on = BTreeMap::from([(SWITCH.to_owned(), "1".to_owned())]);
    if let Err(e) = leonida_core::settings::save_changes(&on) {
        println!("Could not save the choice ({e:#}); on for this start only.");
    }
    // SAFETY: environment updates are thread-safe on Windows, and this runs
    // before the title, Streamline and the GPU's workers start, as the
    // profiles are applied (`crate::graphics`).
    unsafe { std::env::set_var(SWITCH, "1") };
}

/// DLLs in `dir` under the names of Windows' own that leonida.exe loads: a
/// mod's proxy (MFG Unlocker as `winmm.dll`, as its own README says).
fn mods_in(dir: &Path) -> Vec<&'static str> {
    const NAMES: [&str; 7] = [
        "winmm.dll",
        "version.dll",
        "dxgi.dll",
        "d3d12.dll",
        "xinput1_4.dll",
        "dinput8.dll",
        "winhttp.dll",
    ];
    NAMES.into_iter().filter(|n| dir.join(n).is_file()).collect()
}

/// 64 hex digits as 32 bytes.
fn sha256_from_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Offered only to RTX 20 and 30 cards (upstream's cases).
    #[test]
    fn cards_offered_mfg_unlocker() {
        for card in [
            "NVIDIA GeForce RTX 3080 Ti",
            "NVIDIA GeForce RTX 2060 SUPER",
            "NVIDIA GeForce RTX 3050 Laptop GPU",
            "nvidia geforce rtx 3080",
        ] {
            assert!(older_rtx(card), "{card}");
        }
        for card in [
            "NVIDIA GeForce RTX 5070",
            "NVIDIA GeForce RTX 4090",
            "NVIDIA GeForce GTX 1080",
            "NVIDIA RTX A2000",
            "AMD Radeon RX 7900 XTX",
            "",
        ] {
            assert!(!older_rtx(card), "{card}");
        }
    }

    #[test]
    fn no_is_the_default() {
        for yes in ["y\n", "Yes\r\n", " Y "] {
            assert!(says_yes(yes), "{yes:?}");
        }
        for no in ["", "\r\n", "n", "no", "yy", "later"] {
            assert!(!says_yes(no), "{no:?}");
        }
    }

    #[test]
    fn the_pin_is_the_scripts_and_parses() {
        let pin = sha256_from_hex(DLL_SHA256).unwrap();
        assert_eq!((pin[0], pin[31]), (0xA9, 0x8F));
        assert!(GET.1.contains(DLL_SHA256), "the script checks another DLL");
        assert!(GET.1.contains("[switch]$Yes") && GET.1.contains("param("));
        assert!(GET.1.contains("github.com/dashdogy/MFG-Unlocker"));
        assert!(GET.1.contains("leonida\\mfg-unlocker"));
        assert!(GET.1.contains("NVIDIA does not support it"));
        assert_eq!(sha256_from_hex("00"), None);
        assert_eq!(sha256_from_hex(&"zz".repeat(32)), None);
    }

    #[test]
    fn mods_beside_the_program_are_found_by_name() {
        let dir = tempfile::tempdir().unwrap();
        assert!(mods_in(dir.path()).is_empty());
        std::fs::write(dir.path().join("winmm.dll"), b"stand-in").unwrap();
        std::fs::write(dir.path().join("other.dll"), b"stand-in").unwrap();
        assert_eq!(mods_in(dir.path()), ["winmm.dll"]);
    }

    #[test]
    fn the_folder_is_outside_the_repository() {
        if let Ok(home) = home() {
            assert!(home.ends_with("leonida/mfg-unlocker"), "{}", home.display());
        }
    }
}

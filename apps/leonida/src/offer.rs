//! The one-time questions at the start of `leonida run`: where DLSS or FSR 3
//! frame generation is asked for and the vendor's files are not in the data
//! folder yet, whether to fetch them now with `scripts\get-nvidia-dlss.ps1`
//! or `scripts\get-amd-fsr.ps1` (both carried in the executable, run by
//! Windows PowerShell from a fresh folder under %TEMP%). No is the default:
//! nothing is fetched without a "y" or "yes". A "no" is kept (a `declined`
//! file beside where they would go) and not asked again; the scripts can
//! still be run by hand. Only with a terminal to answer in, and
//! only for the default folders (with `LEONIDA_STREAMLINE_DIR` or
//! `LEONIDA_FIDELITYFX_DIR` the files are the player's to place).
//!
//! Ported from trustno1-coder/VIctory-NVIDIA (10862a6, `offer_dlss` and
//! `offer_fsr` in `apps/victory/src/play.rs`).

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use leonida_hle::streamline::{self, Asked};

const GET_DLSS: (&str, &str) =
    ("get-nvidia-dlss.ps1", include_str!("../../../scripts/get-nvidia-dlss.ps1"));
const GET_FSR: (&str, &str) =
    ("get-amd-fsr.ps1", include_str!("../../../scripts/get-amd-fsr.ps1"));

/// NVIDIA's files, where DLSS is asked for (`LEONIDA_DLSS`) and the
/// default folder has no Streamline.
pub(crate) fn dlss(cfg: &leonida_core::Config, asked: &Asked) {
    let default = cfg.data_dir.join(streamline::SUBDIR);
    let there = default.join(streamline::INTERPOSER).is_file();
    if !asked.dlss || asked.dir != default || there {
        return;
    }
    let Some(root) = streamline_root(&default) else { return };
    offer(
        "DLSS needs NVIDIA's Streamline and DLSS files, which are not on this PC yet.",
        "Download them now from NVIDIA's GitHub (asked once; scripts\\get-nvidia-dlss.ps1)?",
        GET_DLSS,
        root,
        &cfg.data_dir.join("nvidia"),
    );
}

/// AMD's files, where FSR 3 frame generation would be used
/// (`LEONIDA_FSR_FRAMEGEN` with DLSS, on a card DLSS Frame Generation does
/// not run on or without `LEONIDA_FRAMEGEN`) and the default folder lacks
/// them.
pub(crate) fn fsr(cfg: &leonida_core::Config, asked: &Asked) {
    let default = cfg.data_dir.join(streamline::FIDELITYFX_SUBDIR);
    if !asked.fsr_framegen || !asked.dlss || asked.fidelityfx_dir != default {
        return;
    }
    if streamline::FIDELITYFX_FILES.iter().all(|f| default.join(f).is_file()) {
        return;
    }
    let card = streamline::adapter_name().unwrap_or_default();
    if !fsr_used(asked.framegen, &card) {
        return;
    }
    offer(
        "FSR 3 frame generation needs AMD's FidelityFX files, which are not on this PC yet.",
        "Download them now from AMD's GitHub (asked once; scripts\\get-amd-fsr.ps1)?",
        GET_FSR,
        &default,
        &cfg.data_dir.join("amd"),
    );
}

/// Whether FSR 3 frame generation is the one used: DLSS Frame Generation is
/// not asked for, or this card does not run it.
fn fsr_used(framegen: bool, card: &str) -> bool {
    !framegen || !streamline::dlss_framegen_card(card)
}

/// The Streamline folder the NVIDIA script fills (`-To`), above `bin\x64`.
fn streamline_root(bin: &Path) -> Option<&Path> {
    bin.parent()?.parent()
}

/// Asks once (default no: the download is the player's choice), then runs
/// the script into `to` on a yes. `home` keeps the `declined` file.
fn offer(why: &str, question: &str, script: (&str, &str), to: &Path, home: &Path) {
    let declined = home.join("declined");
    if declined.is_file() || !std::io::stdin().is_terminal() {
        return;
    }
    println!("{why}");
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return;
    }
    if !says_yes(&answer) {
        println!("Not downloaded; not asked again (the script can be run by hand).");
        let _ = std::fs::create_dir_all(home);
        let _ = std::fs::write(&declined, "declined at the start of leonida run\n");
        return;
    }
    if let Err(e) = run_script(script, to) {
        println!("The download did not finish ({e:#}); starting without these files.");
    }
}

/// Only "y" or "yes", in any case: an empty answer is no.
fn says_yes(answer: &str) -> bool {
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

/// Writes the script to a fresh folder under %TEMP% (the scripts refuse a
/// destination inside the checkout they sit in, taken as their parent
/// folder) and runs it with `-Yes -To <to>`.
pub(crate) fn run_script((name, text): (&str, &str), to: &Path) -> anyhow::Result<()> {
    let temp: PathBuf = std::env::temp_dir().join(format!("leonida-get-{}", std::process::id()));
    let scripts = temp.join("scripts");
    std::fs::create_dir_all(&scripts)?;
    let path = scripts.join(name);
    std::fs::write(&path, text)?;
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&path)
        .arg("-Yes")
        .arg("-To")
        .arg(to)
        .status();
    let _ = std::fs::remove_dir_all(&temp);
    let status = status?;
    anyhow::ensure!(status.success(), "{name} ended with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsr_only_where_dlss_frame_generation_does_not_run() {
        assert!(fsr_used(false, "NVIDIA GeForce RTX 4090"));
        assert!(fsr_used(true, "NVIDIA GeForce RTX 3080"));
        assert!(fsr_used(true, ""));
        assert!(!fsr_used(true, "NVIDIA GeForce RTX 5070 Ti"));
    }

    #[test]
    fn the_nvidia_script_fills_above_bin() {
        let bin = Path::new(r"C:\data").join(streamline::SUBDIR);
        let root = streamline_root(&bin).unwrap();
        assert_eq!(root, Path::new(r"C:\data\nvidia\streamline-v2.14.16"));
    }

    #[test]
    fn no_is_the_default() {
        for yes in ["y\n", "Yes\r\n", " Y "] {
            assert!(says_yes(yes), "{yes:?}");
        }
        for no in ["", "\r\n", "n\n", "no", "later", "yy"] {
            assert!(!says_yes(no), "{no:?}");
        }
    }

    #[test]
    fn the_scripts_are_carried() {
        assert!(GET_DLSS.1.contains("param(") && GET_DLSS.1.contains("[switch]$Yes"));
        assert!(GET_FSR.1.contains("param(") && GET_FSR.1.contains("[switch]$Yes"));
        assert!(GET_FSR.1.contains("GPUOpen-LibrariesAndSDKs/FidelityFX-SDK"));
    }
}

//! Resolve graphics settings before the runtime caches any switches.

use anyhow::Result;
use leonida_core::Config;
use leonida_core::graphics::{Adapter, Plan};

pub fn plan(config: &Config) -> Result<Plan> {
    let environment = leonida_core::graphics::snapshot_environment()?;
    let gpu = match leonida_hle::modules::d3d12x::graphics_adapter() {
        Ok(adapter) => Some(Adapter {
            name: adapter.name,
            vendor: match adapter.vendor_id {
                0x10de => "nvidia",
                0x1002 => "amd",
                0x8086 => "intel",
                _ => "other",
            }
            .into(),
            vram_mb: adapter.vram_mb,
        }),
        Err(error) => {
            eprintln!("graphics adapter unavailable ({error}); matching only general profiles");
            None
        }
    };
    config.graphics_plan(gpu, &environment)
}

/// The executable calls this before loading a title or creating a translation
/// host. Existing GPU consumers keep reading their switches in one place.
#[cfg(windows)]
pub fn initialize(config: &Config) -> Result<()> {
    let plan = plan(config)?;
    apply(&plan);
    let profiles = if plan.profiles.is_empty() {
        "none".to_owned()
    } else {
        plan.profiles.join(", ")
    };
    eprintln!(
        "graphics card: {}; profiles: {profiles}",
        plan.gpu.as_ref().map_or("unknown", |gpu| gpu.name.as_str())
    );
    Ok(())
}

#[cfg(windows)]
fn apply(plan: &Plan) {
    for (key, value) in &plan.settings {
        // Explicit environment values already have precedence in the plan.
        // Leave them alone, including empty values used to disable a switch.
        if plan
            .sources
            .get(key)
            .is_some_and(|source| source == "environment")
        {
            continue;
        }
        // SAFETY: environment updates are thread-safe on Windows. This bridge
        // is Windows-only and runs before the GPU's workers and OnceLocks; the
        // resolver rejects NULs and invalid variable names before this point.
        unsafe { std::env::set_var(key, value) };
    }
}

#[cfg(not(windows))]
pub fn initialize(config: &Config) -> Result<()> {
    // The native Linux frontend can inspect files but cannot run the Windows
    // GPU backend. Validate its settings without mutating a Unix environment.
    let _ = plan(config)?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn startup_applies_settings_without_replacing_explicit_values() {
        const CHILD: &str = "LEONIDA_GRAPHICS_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let config = Config::load().unwrap();
            initialize(&config).unwrap();
            assert_eq!(std::env::var("LEONIDA_ASYNC_SUBMIT").unwrap(), "0");
            assert_eq!(std::env::var("LEONIDA_QUEUE2").unwrap(), "2");
            assert_eq!(std::env::var("LEONIDA_INDIRECT_GATHER").unwrap(), "1");
            assert_eq!(std::env::var("LEONIDA_JOIN_PARTS").unwrap(), "");
            assert!(std::env::var_os("LEONIDA_CARD_WINDOWS_MB").is_none());
            eprintln!("graphics startup: config, profile and explicit overrides applied");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("settings.toml");
        std::fs::write(&config, "[graphics]\nasync_submit = false\nqueue2 = 3\n").unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap());
        for (key, _) in std::env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("LEONIDA_")
            {
                child.env_remove(key);
            }
        }
        let output = child
            .args([
                "--exact",
                "graphics::tests::startup_applies_settings_without_replacing_explicit_values",
                "--nocapture",
            ])
            .env(CHILD, "1")
            // Not the player's own settings file.
            .env("LEONIDA_SAVED_SETTINGS", "0")
            .env("LEONIDA_CONFIG", config)
            .env("LEONIDA_QUEUE2", "2")
            .env("LEONIDA_JOIN_PARTS", "")
            .current_dir(dir.path())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert!(
            stderr.contains("graphics startup: config, profile and explicit overrides applied")
        );
    }
}

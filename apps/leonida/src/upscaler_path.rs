//! DLSS (`LEONIDA_DLSS`): makes the title render through its FidelityFX
//! upscaler path (renderer 7, as Series S does) with the scene at the DLSS
//! mode's scale of the output, so that DLSS can take its upscale dispatches
//! over (`leonida_hle::modules::fidelityfx`). Stock Series X uses its own
//! temporal pass instead and never loads its upscaler.
//!
//! Every site is checked first: the BLAKE3 of its bytes in the image must
//! be the one of the build the sites were found in (only the hashes are
//! here), and the two scales must be 2/3. Nothing is written if one
//! differs, and the title then runs as before.
//!
//! Sites from trustno1-coder/VIctory-NVIDIA (959dcaf,
//! `apps/victory/src/native_1440p.rs`, `prepare_upscaler`, work of
//! readmet3xt-lab), there from DexNom's fork (DexNom/VIctory,
//! `prepare_fsr_4k`), both GPL-3.0-or-later. The build they were found in is
//! not the only one: on another build the hashes differ and this refuses.

use anyhow::{Context, Result, ensure};
use leonida_hle::streamline::plan::Mode;
use leonida_loader::PreparedImage;

/// A stretch of the image, by RVA, and the BLAKE3 of its stock bytes.
#[derive(Clone, Copy, Debug)]
struct Site {
    rva: u32,
    len: usize,
    stock: &'static str,
}

/// Where the upscaler path is chosen and sized in one build.
#[derive(Clone, Copy, Debug)]
struct Sites {
    /// The renderer the title picks: its own temporal pass (6) on Series X.
    renderer: Site,
    /// The FidelityFX path's render size: the console check noted as the
    /// FidelityFX quality mode (14 bytes), then the native branch's load of
    /// 1.0 (10) and the scaled branch's load of the scale (8).
    sizing: Site,
    /// The scale the scaled branch loads (shared with other code: only
    /// checked).
    upscaler_scale: u32,
    /// The scene scale of the title's own temporal pass.
    temporal_scale: u32,
}

/// The retail build VIctory-NVIDIA patched (the SHA-256 of its executable
/// starts b99f876b).
const RETAIL: Sites = Sites {
    renderer: Site {
        rva: 0x007e_9421,
        len: 8,
        stock: "f0633abf37df17124108b1e37be64645bd4b2412b0524c2f89334f22bd948464",
    },
    sizing: Site {
        rva: 0x028e_f69a,
        len: 32,
        stock: "ceda86f4b19b57bf64f6b562c337bea6ad18d4359f3aea2c98adc4a3fb5b7eaa",
    },
    upscaler_scale: 0x0615_d95c,
    temporal_scale: 0x028f_1e7e,
};

// Within the sizing site: the store of the noted mode and its length, then
// the two loads.
const STOCK_STORE: usize = 3;
const STORE_LEN: usize = 7;
const NATIVE_LOAD: usize = 14;
const SCALED_LOAD: usize = 24;

/// Patches `image` for `mode` on the retail build; an error (and `image` as
/// it was) on any other.
pub fn patch(image: &mut PreparedImage, mode: Mode) -> Result<()> {
    let info = &image.info;
    let offset = |rva: u32| info.rva_to_file_offset(rva).map(|o| o as usize);
    apply(&mut image.bytes, &offset, &RETAIL, mode)
}

fn apply(
    bytes: &mut [u8],
    offset: &dyn Fn(u32) -> Option<usize>,
    sites: &Sites,
    mode: Mode,
) -> Result<()> {
    let at = |rva: u32, len: usize| -> Result<usize> {
        let start = offset(rva).with_context(|| format!("RVA {rva:#x} is not in the file"))?;
        ensure!(start + len <= bytes.len(), "RVA {rva:#x} runs past the file");
        Ok(start)
    };
    for (name, site) in [("renderer selection", sites.renderer), ("render size", sites.sizing)] {
        let start = at(site.rva, site.len)?;
        let hash = blake3::hash(&bytes[start..start + site.len]);
        ensure!(
            hash.to_hex().as_str() == site.stock,
            "the {name} is not the one of the known build (another build, or patched already)"
        );
    }
    for (name, rva) in [("upscaler", sites.upscaler_scale), ("temporal", sites.temporal_scale)] {
        let start = at(rva, 4)?;
        let scale = f32::from_le_bytes(bytes[start..start + 4].try_into()?);
        ensure!(scale == 2.0 / 3.0, "unexpected {name} scale {scale} (stock 2/3)");
    }

    let renderer = at(sites.renderer.rva, sites.renderer.len)?;
    let sizing = at(sites.sizing.rva, sites.sizing.len)?;
    let temporal = at(sites.temporal_scale, 4)?;
    // Renderer 7 every time it is asked.
    bytes[renderer..renderer + 8].copy_from_slice(&[0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
    // The mode in eax, the title's own store of it (moved two bytes on, so
    // it is read from the image and not written here), then on to the
    // scaled branch.
    let store: [u8; STORE_LEN] = bytes[sizing + STOCK_STORE..][..STORE_LEN].try_into()?;
    bytes[sizing..sizing + 5].copy_from_slice(&[0xb8, mode.ffx_mode(), 0, 0, 0]);
    bytes[sizing + 5..sizing + 5 + STORE_LEN].copy_from_slice(&store);
    bytes[sizing + 5 + STORE_LEN..sizing + NATIVE_LOAD].copy_from_slice(&[0xeb, 0x0a]);
    if mode != Mode::Quality {
        // The native branch is no longer reached: its load's operand becomes
        // a literal of ours, which the scaled branch loads instead.
        let literal = NATIVE_LOAD + 4;
        bytes[sizing + literal..sizing + literal + 4].copy_from_slice(&mode.scale().to_le_bytes());
        let displacement = literal as i32 - (SCALED_LOAD + 8) as i32;
        let operand = sizing + SCALED_LOAD + 4;
        bytes[operand..operand + 4].copy_from_slice(&displacement.to_le_bytes());
    }
    bytes[temporal..temporal + 4].copy_from_slice(&mode.scale().to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up image: filler where the sites are, the scales at 2/3, and
    /// the hashes of that filler as the stock ones. File offsets are RVAs.
    fn synthetic() -> (Vec<u8>, Sites) {
        let mut bytes: Vec<u8> = (0..0x100u32).map(|i| (i * 7 + 3) as u8).collect();
        let renderer = 0x10;
        let sizing = 0x40;
        let (upscaler_scale, temporal_scale) = (0x80, 0xa0);
        for at in [upscaler_scale, temporal_scale] {
            bytes[at..at + 4].copy_from_slice(&(2.0f32 / 3.0).to_le_bytes());
        }
        let leak = |s: blake3::Hash| -> &'static str { s.to_hex().to_string().leak() };
        let sites = Sites {
            renderer: Site {
                rva: renderer as u32,
                len: 8,
                stock: leak(blake3::hash(&bytes[renderer..renderer + 8])),
            },
            sizing: Site {
                rva: sizing as u32,
                len: 32,
                stock: leak(blake3::hash(&bytes[sizing..sizing + 32])),
            },
            upscaler_scale: upscaler_scale as u32,
            temporal_scale: temporal_scale as u32,
        };
        (bytes, sites)
    }

    fn identity(rva: u32) -> Option<usize> {
        Some(rva as usize)
    }

    fn f32_at(bytes: &[u8], at: usize) -> f32 {
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    /// Each mode: renderer 7, the mode noted, the jump landing on the scaled
    /// load, which then loads the mode's scale, the temporal scale the same,
    /// the shared scale untouched; patched once, refused.
    #[test]
    fn a_matching_image_takes_the_upscaler_path() {
        let modes = [
            Mode::Dlaa,
            Mode::Quality,
            Mode::Balanced,
            Mode::Performance,
            Mode::UltraPerformance,
        ];
        for mode in modes {
            let (mut bytes, sites) = synthetic();
            apply(&mut bytes, &identity, &sites, mode).unwrap();
            let renderer = sites.renderer.rva as usize;
            assert_eq!(bytes[renderer..renderer + 8], [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
            let sizing = sites.sizing.rva as usize;
            assert_eq!(bytes[sizing..sizing + 5], [0xb8, mode.ffx_mode(), 0, 0, 0]);
            let stock = &synthetic().0[sizing + STOCK_STORE..][..STORE_LEN];
            assert_eq!(&bytes[sizing + 5..][..STORE_LEN], stock);
            assert_eq!(bytes[sizing + 12..sizing + 14], [0xeb, 0x0a]);
            assert_eq!(NATIVE_LOAD + 0x0a, SCALED_LOAD);
            let operand = sizing + SCALED_LOAD + 4;
            let displacement = i32::from_le_bytes(bytes[operand..operand + 4].try_into().unwrap());
            let loaded = (sizing + SCALED_LOAD + 8).wrapping_add_signed(displacement as isize);
            if mode == Mode::Quality {
                // The stock operand is kept (here filler; in the title it
                // reaches the shared 2/3).
                assert_eq!(bytes[operand..operand + 4], synthetic().0[operand..operand + 4]);
            } else {
                assert_eq!(f32_at(&bytes, loaded), mode.scale());
            }
            assert_eq!(f32_at(&bytes, sites.temporal_scale as usize), mode.scale());
            assert_eq!(f32_at(&bytes, sites.upscaler_scale as usize), 2.0 / 3.0);
            assert!(apply(&mut bytes, &identity, &sites, mode).is_err());
        }
    }

    /// One byte off in either site, or a scale that is not 2/3: refused, and
    /// nothing written.
    #[test]
    fn another_build_is_left_alone() {
        let (stock, sites) = synthetic();
        for at in [
            sites.renderer.rva as usize + 7,
            sites.sizing.rva as usize + 31,
            sites.upscaler_scale as usize,
            sites.temporal_scale as usize + 3,
        ] {
            let mut bytes = stock.clone();
            bytes[at] ^= 1;
            let changed = bytes.clone();
            assert!(apply(&mut bytes, &identity, &sites, Mode::Balanced).is_err());
            assert_eq!(bytes, changed);
        }
        let mut short = stock[..0x60].to_vec();
        assert!(apply(&mut short, &identity, &sites, Mode::Quality).is_err());
        assert!(apply(&mut stock.clone(), &|_| None, &sites, Mode::Quality).is_err());
    }

    /// With an executable named by `LEONIDA_TEST_IMAGE` (never part of the
    /// repository): whether the known sites match it. Prints only that.
    #[test]
    #[ignore = "needs LEONIDA_TEST_IMAGE"]
    fn the_known_sites_against_an_image() {
        let Some(path) = std::env::var_os("LEONIDA_TEST_IMAGE") else { return };
        let bytes = std::fs::read(path).unwrap();
        let mut image = leonida_loader::prepare(&bytes).unwrap();
        match patch(&mut image, Mode::Quality) {
            Ok(()) => println!("upscaler path: the known sites match this image"),
            Err(e) => println!("upscaler path: refused: {e:#}"),
        }
    }
}

//! Adapter metadata for startup settings, before any GPU switches are cached.

#[derive(Debug, Clone)]
pub struct Description {
    pub name: String,
    pub vendor_id: u32,
    pub vram_mb: u64,
}

#[cfg(windows)]
pub(crate) fn warp_requested() -> bool {
    cfg!(feature = "diagnostics") && std::env::var_os("LEONIDA_D3D12_WARP").is_some()
}

/// Describes the adapter that `host::create` will use, without making the host
/// device or starting its workers. A failed probe leaves automatic settings
/// limited to profiles that do not require a particular card.
#[cfg(windows)]
pub fn describe() -> Result<Description, String> {
    use windows::Win32::Graphics::Dxgi::*;
    use windows::core::Interface;

    let probe = || -> windows::core::Result<Description> {
        // SAFETY: DXGI queries own their returned interfaces; no title pointers
        // or D3D12 device state are accessed.
        unsafe {
            let adapter: IDXGIAdapter1 = if warp_requested() {
                crate::host::warp_adapter()?.cast()?
            } else {
                let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
                // D3D12CreateDevice(None), used by the host, selects this first
                // adapter, not necessarily the one with the most memory.
                factory.EnumAdapters(0)?.cast()?
            };
            let desc = adapter.GetDesc1()?;
            let end = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
            Ok(Description {
                name: String::from_utf16_lossy(&desc.Description[..end]),
                vendor_id: desc.VendorId,
                vram_mb: (desc.DedicatedVideoMemory as u64) >> 20,
            })
        }
    };
    probe().map_err(|error| error.to_string())
}

#[cfg(not(windows))]
pub fn describe() -> Result<Description, String> {
    Err("Direct3D adapter discovery requires Windows or Wine".into())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore = "requires a physical Direct3D 12 adapter"]
    fn profile_metadata_matches_the_default_hardware_device() {
        use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0;
        use windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};
        use windows::Win32::Graphics::Dxgi::*;

        assert!(std::env::var_os("LEONIDA_D3D12_WARP").is_none());
        let selected = super::describe().expect("a physical adapter is required");
        // SAFETY: creates an ordinary default D3D12 device and queries its
        // adapter identity. No title code, pointers, commands or assets are used.
        unsafe {
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(None, D3D_FEATURE_LEVEL_12_0, &mut device).unwrap();
            let device = device.unwrap();
            let factory: IDXGIFactory4 = CreateDXGIFactory1().unwrap();
            let adapter: IDXGIAdapter1 =
                factory.EnumAdapterByLuid(device.GetAdapterLuid()).unwrap();
            let actual = adapter.GetDesc1().unwrap();
            assert_eq!(actual.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32, 0);
            let end = actual
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(128);
            assert_eq!(
                selected.name,
                String::from_utf16_lossy(&actual.Description[..end])
            );
            assert_eq!(selected.vendor_id, actual.VendorId);
            assert_eq!(selected.vram_mb, (actual.DedicatedVideoMemory as u64) >> 20);
        }
        eprintln!("graphics adapter: metadata matches the physical default D3D12 device");
    }
}

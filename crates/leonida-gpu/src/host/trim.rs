// Allocator trim: memory stack by skornuta (skornuta/victory-memory-stack 57fb136, GPL-3.0),
// patches 13 and 17.

use super::{host, now_ms, retire};
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, ID3D12CommandAllocator,
};

/// Whether host command allocators are taken anew now and then
/// (`LEONIDA_ALLOCATOR_TRIM=1`; off by default), and after how many
/// seconds. A D3D12 command allocator never gives back the memory of the
/// most its list ever recorded into it: a list that once took a loading
/// frame's commands keeps that for the run, and upstream 1,600 of them
/// (host lists and their preludes) held about 2.3 GB of private memory
/// beside the card's own. With the switch a list's reset (and a prelude's,
/// before it records again: the only moments its allocator holds nothing
/// that runs) takes a new allocator when the one it has is
/// `LEONIDA_ALLOCATOR_TRIM_S` seconds old (30, 5 to 3600), and the old one
/// goes behind the submissions still running ([`retire`]). Idle pooled
/// lists not taken for that long go too (`crate::list`), and every ten
/// seconds the spare scratch chunks nobody took all that while
/// ([`Spare::trim`]). A title list executed again without a reset keeps
/// its allocator: only resets swap.
pub(crate) fn allocator_trim() -> Option<u64> {
    static S: OnceLock<Option<u64>> = OnceLock::new();
    *S.get_or_init(|| {
        std::env::var_os("LEONIDA_ALLOCATOR_TRIM").is_some_and(|v| v == "1").then(|| {
            std::env::var("LEONIDA_ALLOCATOR_TRIM_S")
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(30u64)
                .clamp(5, 3600)
        })
    })
}

/// Since the start: host list allocators and prelude allocators taken anew
/// ([`allocator_trim`]).
pub(crate) static ALLOCATOR_TRIMS: [std::sync::atomic::AtomicU64; 2] = [const { std::sync::atomic::AtomicU64::new(0) }; 2];

/// With [`allocator_trim`]: `allocator`, made at `made`, is replaced by a
/// new one if it is old enough, and the old one is retired. Whether it was
/// (the new one needs no reset). To be called only where the allocator's
/// last commands have run. Without the switch: nothing, false.
pub(super) fn allocator_refresh(allocator: &mut ID3D12CommandAllocator, made: &mut u64, count: &std::sync::atomic::AtomicU64) -> bool {
    let Some(secs) = allocator_trim() else {
        return false;
    };
    let now = now_ms();
    if now.saturating_sub(*made) < secs * 1000 {
        return false;
    }
    let Some(host) = host() else {
        return false;
    };
    // SAFETY: plain object creation.
    let fresh: windows::core::Result<ID3D12CommandAllocator> =
        unsafe { host.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) };
    match fresh {
        Ok(fresh) => {
            retire(std::mem::replace(allocator, fresh));
            *made = now;
            count.fetch_add(1, Ordering::Relaxed);
            true
        }
        Err(_) => {
            // No new one now: the old one serves on, and is asked again later.
            *made = now;
            false
        }
    }
}

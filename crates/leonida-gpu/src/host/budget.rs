// Indirect budgets packed: memory stack by skornuta (skornuta/victory-memory-stack 57fb136,
// GPL-3.0), patch 23.

use super::{host, name, retire};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

/// Whether host lists' indirect-work budgets in memory the CPU maps share
/// buffers (`LEONIDA_BUDGET_PACK=1`; off by default). Those words are 256
/// bytes, and each list made a shared buffer of its own for them: a
/// committed buffer takes 64 KiB, and 602 of them held 37 MB in the first
/// area upstream. With the switch a list's mapped budget is a 256-byte slot
/// of a 64 KiB buffer of [`BUDGET_SLABS`], zeroed when taken, given back
/// behind the submissions running when its list goes ([`retire`]).
///
/// Only the mapped words are packed. Shaders reach them through their
/// address (a root view: no barrier names the buffer), and with the card's
/// copy ([`card_budget`], on by default) the list only copies its notes
/// into them at its end, with the buffer promoted from common as every
/// buffer is, never moved between states by a barrier of ours. The card's
/// copy is moved between states by the list's own barriers, so it stays a
/// buffer of its own: other lists sharing it would see those barriers.
pub(super) fn budget_pack() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LEONIDA_BUDGET_PACK").is_some_and(|v| v == "1"))
}

/// The bytes of one budget, and of one buffer of [`BUDGET_SLABS`].
const BUDGET_SLOT: u64 = 256;
const BUDGET_SLAB: u64 = 64 << 10;

/// The packed budgets' buffers (buffer, mapping), and the free slots
/// (buffer by its place, offset), taken from the end.
#[allow(clippy::type_complexity)]
static BUDGET_SLABS: Mutex<(Vec<(ID3D12Resource, usize)>, Vec<(usize, u64)>)> = Mutex::new((Vec::new(), Vec::new()));

/// A host list's indirect-work budget in memory the CPU maps: a buffer of
/// its own (as without [`budget_pack`]), or a slot of a shared one.
pub(super) enum Budget {
    Own(ID3D12Resource, usize),
    Packed(BudgetSlot),
}

/// A slot of [`BUDGET_SLABS`]: its buffer (held, for the copy of the
/// card's notes into it), where in it, and the slot's mapped address. Given
/// back when dropped, behind the submissions running then.
pub(super) struct BudgetSlot {
    buffer: ID3D12Resource,
    slab: usize,
    offset: u64,
    data: usize,
}

/// Gives a slot back when dropped ([`retire`] drops it once the
/// submissions running at its list's end have finished).
struct BudgetFree(usize, u64);

impl Drop for BudgetFree {
    fn drop(&mut self) {
        BUDGET_SLABS.lock().unwrap_or_else(|e| e.into_inner()).1.push((self.0, self.1));
    }
}

impl Drop for BudgetSlot {
    fn drop(&mut self) {
        retire(BudgetFree(self.slab, self.offset));
    }
}

impl Budget {
    pub(super) fn new() -> Option<Budget> {
        if !budget_pack() {
            let (b, d) = host()?.shared_buffer(BUDGET_SLOT)?;
            return Some(Budget::Own(b, d as usize));
        }
        let mut slabs = BUDGET_SLABS.lock().unwrap_or_else(|e| e.into_inner());
        if slabs.1.is_empty() {
            let (buffer, data) = host()?.shared_buffer(BUDGET_SLAB)?;
            name(&buffer, || "indirect budgets".into());
            let at = slabs.0.len();
            slabs.0.push((buffer, data as usize));
            // Taken from the end: the buffer's first slot first.
            slabs.1.extend((0..BUDGET_SLAB / BUDGET_SLOT).rev().map(|k| (at, k * BUDGET_SLOT)));
        }
        let (slab, offset) = slabs.1.pop()?;
        let (buffer, data) = &slabs.0[slab];
        let (buffer, data) = (buffer.clone(), data + offset as usize);
        drop(slabs);
        // A slot given back holds its last list's words: zeroed, as a new
        // buffer is.
        // SAFETY: the slot's 256 mapped bytes, ours alone now (its last
        // list's submissions have run).
        unsafe { std::ptr::write_bytes(data as *mut u8, 0, BUDGET_SLOT as usize) };
        Some(Budget::Packed(BudgetSlot { buffer, slab, offset, data }))
    }

    /// The mapped address of the budget's first word.
    pub(super) fn data(&self) -> usize {
        match self {
            Budget::Own(_, data) => *data,
            Budget::Packed(slot) => slot.data,
        }
    }

    /// The buffer that holds the budget, and where in it.
    pub(super) fn place(&self) -> (&ID3D12Resource, u64) {
        match self {
            Budget::Own(buffer, _) => (buffer, 0),
            Budget::Packed(slot) => (&slot.buffer, slot.offset),
        }
    }
}

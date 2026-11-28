//! What this card does for frame generation, said at the top of the picture
//! for the first seconds of a run (`crate::streamline::notice`), in every
//! build: white text on a black box, copied onto the swap chain's buffer
//! behind the title's picture, so saved frames and the frame recorder see the
//! title's own. From VIctory-NVIDIA's numbers panel; the F3 panel itself is
//! `crate::panel` now.

use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::font::{colours, glyph};
use super::{Host, HostList, transition};

/// A character's dots, and with the space beside and below it.
const GLYPH: (u32, u32) = (5, 7);
const CELL: (u32, u32) = (6, 9);
/// The box's rim in dots.
const RIM: u32 = 3;

/// The texts are put into one of these in turn: the card reads the others,
/// at most half a second old.
const UPLOADS: usize = 4;

/// A box's characters in a line, and lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Shape {
    columns: u32,
    lines: u32,
}

/// The box's size in pixels, a dot being `scale` of them each way.
const fn size(shape: Shape, scale: u32) -> (u32, u32) {
    ((shape.columns * CELL.0 + 2 * RIM) * scale, (shape.lines * CELL.1 + 2 * RIM) * scale)
}

/// Pixels to a dot for a picture that high: a 720-line screenshot of it
/// shows one and a third.
fn scale(height: u32) -> u32 {
    (height / 540).max(1)
}

/// Whether a dot of the picture of `lines` is white.
fn lit(lines: &[String], shape: Shape, x: u32, y: u32) -> bool {
    let (Some(x), Some(y)) = (x.checked_sub(RIM), y.checked_sub(RIM)) else {
        return false;
    };
    if x % CELL.0 >= GLYPH.0 || y % CELL.1 >= GLYPH.1 || x / CELL.0 >= shape.columns {
        return false;
    }
    let Some(&c) = lines.get((y / CELL.1) as usize).and_then(|l| l.as_bytes().get((x / CELL.0) as usize)) else {
        return false;
    };
    (glyph(c.to_ascii_uppercase())[(y % CELL.1) as usize] >> (GLYPH.0 - 1 - x % CELL.0)) & 1 != 0
}

/// The box as rows of `pitch` bytes.
fn painted(lines: &[String], shape: Shape, scale: u32, colours: [&[u8]; 2], pitch: usize) -> Vec<u8> {
    let (width, height) = size(shape, scale);
    let bytes = colours[0].len();
    let mut out = vec![0; pitch * height as usize];
    for y in 0..height / scale {
        let top = pitch * (y * scale) as usize;
        for x in 0..width / scale {
            let colour = colours[usize::from(lit(lines, shape, x, y))];
            for i in 0..scale {
                let at = top + (x * scale + i) as usize * bytes;
                out[at..at + bytes].copy_from_slice(colour);
            }
        }
        for i in 1..scale as usize {
            out.copy_within(top..top + pitch, top + i * pitch);
        }
    }
    out
}

/// The buffers of a box that size and format.
struct Buffers {
    shape: Shape,
    format: DXGI_FORMAT,
    scale: u32,
    pitch: u32,
    uploads: Vec<(ID3D12Resource, *mut u8)>,
    card: ID3D12Resource,
    /// The upload written last.
    turn: usize,
    /// It is not in the card's buffer yet.
    fresh: bool,
}

// SAFETY: free-threaded resources, and memory only written under the presenter's lock.
unsafe impl Send for Buffers {}

impl Buffers {
    fn new(host: &Host, shape: Shape, format: DXGI_FORMAT, scale: u32) -> Option<Buffers> {
        let (width, height) = size(shape, scale);
        let pitch = (width * colours(format)?[0].len() as u32).next_multiple_of(256);
        let bytes = u64::from(pitch) * u64::from(height);
        let uploads = (0..UPLOADS).map(|_| host.upload_buffer(bytes)).collect::<Option<Vec<_>>>()?;
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: bytes,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            ..Default::default()
        };
        let mut card: Option<ID3D12Resource> = None;
        // SAFETY: plain creation of a buffer.
        unsafe {
            host.device
                .CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COMMON, None, &mut card)
                .ok()?;
        }
        Some(Buffers { shape, format, scale, pitch, uploads, card: card?, turn: 0, fresh: false })
    }

    fn write(&mut self, lines: &[String]) {
        let Some(colours) = colours(self.format) else { return };
        let picture = painted(lines, self.shape, self.scale, colours, self.pitch as usize);
        self.turn = (self.turn + 1) % UPLOADS;
        // SAFETY: a mapped upload buffer of the picture's size, which the card read a second or more ago.
        unsafe { std::ptr::copy_nonoverlapping(picture.as_ptr(), self.uploads[self.turn].1, picture.len()) };
        self.fresh = true;
    }

    /// Records the copy of the box onto `back` (of `width` x `height`, in the
    /// state to be copied into), its upper left corner at `at`.
    fn copy(&mut self, list: &ID3D12GraphicsCommandList, back: &ID3D12Resource, at: (u32, u32), (width, height): (u32, u32)) {
        let (w, h) = size(self.shape, self.scale);
        if at.0 >= width || at.1 >= height {
            return;
        }
        // SAFETY: an open list and live resources; the box lies within both.
        unsafe {
            if std::mem::take(&mut self.fresh) {
                list.CopyResource(&self.card, &self.uploads[self.turn].0);
                list.ResourceBarrier(&[transition(&self.card, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            }
            let from = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&self.card),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: self.format,
                            Width: w,
                            Height: h,
                            Depth: 1,
                            RowPitch: self.pitch,
                        },
                    },
                },
            };
            let to = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(back),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
            };
            let part = D3D12_BOX { left: 0, top: 0, front: 0, right: w.min(width - at.0), bottom: h.min(height - at.1), back: 1 };
            list.CopyTextureRegion(&to, at.0, at.1, 0, &from, Some(&part));
        }
    }
}

/// How long the start's notice stays.
const NOTICE_FOR: Duration = Duration::from_secs(30);
/// The longest notice line, in characters.
const NOTICE_COLUMNS: u32 = 52;

/// What this card does for frame generation, said at the top of the picture
/// for the first [`NOTICE_FOR`] of a run (`crate::streamline::notice`); in
/// every build, nothing without a notice.
pub(super) struct Notice {
    lines: Vec<String>,
    shape: Shape,
    since: Option<Instant>,
    buffers: Option<Buffers>,
    written: bool,
    done: bool,
}

impl Notice {
    pub(super) fn new() -> Notice {
        let lines: Vec<String> =
            crate::streamline::notice().into_iter().map(|l| l.to_ascii_uppercase()).collect();
        let columns = lines.iter().map(|l| l.len() as u32).max().unwrap_or(0).min(NOTICE_COLUMNS);
        let shape = Shape { columns, lines: lines.len() as u32 };
        Notice { done: lines.is_empty(), lines, shape, since: None, buffers: None, written: false }
    }

    /// Records the copy of the notice onto `back` (behind the picture), at
    /// the top in the middle, while it is due.
    pub(super) fn draw(&mut self, host: &Host, list: &HostList, back: &ID3D12Resource, (width, height): (u32, u32), format: DXGI_FORMAT) {
        if self.done {
            return;
        }
        let since = *self.since.get_or_insert_with(Instant::now);
        if since.elapsed() > NOTICE_FOR {
            // (Its buffers stay: the card may still read them.)
            std::mem::forget(self.buffers.take());
            self.done = true;
            return;
        }
        #[cfg(feature = "diagnostics")]
        let _ours = crate::replay::record::unrecorded();
        let scale = scale(height);
        if self.buffers.as_ref().is_none_or(|b| (b.format, b.scale) != (format, scale)) {
            std::mem::forget(self.buffers.take());
            self.buffers = Buffers::new(host, self.shape, format, scale);
            self.written = false;
            if self.buffers.is_none() {
                self.done = true;
                return;
            }
        }
        let Some(buffers) = &mut self.buffers else { return };
        if !std::mem::replace(&mut self.written, true) {
            buffers.write(&self.lines);
        }
        let (w, _) = size(self.shape, scale);
        let at = (width.saturating_sub(w) / 2, 24 * scale);
        buffers.copy(&list.list, back, at, (width, height));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Every character the lines use has its own picture, and no two look
    /// alike.
    #[test]
    fn characters_differ() {
        let used: Vec<u8> = (b'0'..=b'9').chain(b'A'..=b'Z').chain(*b"./- ").collect();
        for (i, a) in used.iter().enumerate() {
            assert_ne!(glyph(*a), glyph(b'#'), "{} has a picture", *a as char);
            assert!(glyph(*a).iter().all(|row| row >> GLYPH.0 == 0));
            for b in &used[i + 1..] {
                assert_ne!(glyph(*a), glyph(*b), "{} {}", *a as char, *b as char);
            }
        }
    }

    #[test]
    fn a_box_is_painted() {
        let shape = Shape { columns: 4, lines: 2 };
        let lines = vec!["I".to_owned(), " i".to_owned()];
        for scale in [1, 2, 4] {
            let (width, height) = size(shape, scale);
            let pitch = (width * 4).next_multiple_of(256) as usize;
            let colours = colours(DXGI_FORMAT_R10G10B10A2_UNORM).unwrap();
            let picture = painted(&lines, shape, scale, colours, pitch);
            assert_eq!(picture.len(), pitch * height as usize);
            let at = |x: u32, y: u32| &picture[y as usize * pitch + x as usize * 4..][..4];
            // The rim is black, the first letter's top row lit in its middle.
            assert_eq!(at(0, 0), colours[0]);
            assert_eq!(at((RIM + 2) * scale, RIM * scale), colours[1]);
            let corner = ((RIM + 2) * scale + scale - 1, RIM * scale + scale - 1);
            assert_eq!(at(corner.0, corner.1), colours[1]);
            assert_eq!(at(RIM * scale, RIM * scale), colours[0]);
            // Small letters are shown as capitals, a line and a cell on.
            assert_eq!(at((RIM + CELL.0 + 2) * scale, (RIM + CELL.1) * scale), colours[1]);
            // Nothing past a line's end.
            assert_eq!(at((RIM + CELL.0 * 2 + 2) * scale, (RIM + CELL.1) * scale), colours[0]);
        }
    }
}

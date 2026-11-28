//! The F3 panel (`crate::panel`) and the F1 menu (`crate::menu`), copied
//! onto the swap chain's back buffer right after the title's frame, the way
//! the frame rate counter is (`crate::fpsbox`): the title's own frame, and
//! everything read back from it, stay as they were.
//!
//! A picture becomes pixels in a buffer the card copies from only when it
//! changes (the panel once a second, the menu on a key), and that buffer is
//! never written again: a present still in the air reads what it was
//! recorded with. A buffer no picture has used for [`KEEP`] presents is let
//! go.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::canvas::Picture;

/// Presents a buffer is kept after it was last copied from: far more than
/// presents can be in the air.
const KEEP: u64 = 120;

struct Made {
    buffer: ID3D12Resource,
    width: u32,
    height: u32,
    pitch: u32,
    /// The present it was last copied from in.
    used: u64,
}

// SAFETY: a free-threaded D3D12 object and plain numbers.
unsafe impl Send for Made {}

struct Boxes {
    presents: u64,
    made: HashMap<u64, Made>,
}

static BOXES: Mutex<Option<Boxes>> = Mutex::new(None);

/// For the F3 panel and the F1 menu: the frame generation that runs, and
/// frames shown for each one rendered (VIctory-NVIDIA's
/// `streamline::framegen_kind` and `shown_per_frame`). None while it shows
/// one frame each.
pub(crate) fn framegen_running() -> Option<(&'static str, u32)> {
    let kind = crate::streamline::framegen_kind()?;
    let times = crate::streamline::shown_per_frame();
    (times > 1).then_some((kind, times))
}

/// Copies the panel and the menu, those that show, onto `back`, a back
/// buffer of `width` by `height` pixels of `format` that `list` holds as a
/// copy's destination. Also writes the settings file once the menu's
/// changes have been quiet for a while.
pub(crate) fn draw(
    host: &crate::host::Host,
    list: &ID3D12GraphicsCommandList,
    back: &ID3D12Resource,
    (width, height, format): (u32, u32, DXGI_FORMAT),
) {
    crate::menu::save_when_quiet();
    // Not the title's: the frame recorder leaves it out.
    #[cfg(feature = "diagnostics")]
    let _ours = crate::replay::record::unrecorded();
    let mut wanted = Vec::new();
    if let Some(picture) = crate::panel::picture(height) {
        let margin = crate::panel::margin(height);
        wanted.push((margin, margin, picture));
    }
    if let Some(placed) = crate::menu::picture((width, height)) {
        wanted.push(placed);
    }
    let mut boxes = BOXES.lock().unwrap_or_else(|e| e.into_inner());
    let boxes = boxes.get_or_insert_with(|| Boxes {
        presents: 0,
        made: HashMap::new(),
    });
    boxes.presents += 1;
    let now = boxes.presents;
    for (x, y, picture) in wanted {
        if x + picture.width > width || y + picture.height > height {
            continue;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (format.0, &picture).hash(&mut hasher);
        let key = hasher.finish();
        let m = match boxes.made.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let Some(made) = make(host, &picture, format) else {
                    continue;
                };
                e.insert(made)
            }
        };
        m.used = now;
        copy(list, back, m, (x, y), format);
    }
    boxes.made.retain(|_, m| now - m.used <= KEEP);
}

/// A buffer holding `picture`'s pixels in `format`, laid out for a copy.
fn make(host: &crate::host::Host, picture: &Picture, format: DXGI_FORMAT) -> Option<Made> {
    let bytes = crate::fpsbox::pixel_bytes(format)?;
    let (w, h) = (picture.width as usize, picture.height as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let pitch = (w * bytes).next_multiple_of(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize);
    let (buffer, data) = host.shared_buffer((pitch * h) as u64)?;
    // SAFETY: the buffer's own mapped bytes, `pitch * h` of them.
    let out = unsafe { std::slice::from_raw_parts_mut(data, pitch * h) };
    let mut colours: HashMap<[u8; 3], [u8; 8]> = HashMap::new();
    for (i, rgb) in picture.render().into_iter().enumerate() {
        let pixel = colours.entry(rgb).or_insert_with(|| {
            let mut pixel = [0u8; 8];
            let [r, g, b] = rgb.map(|v| f32::from(v) / 255.0);
            crate::fpsbox::put(format, (r, g, b), &mut pixel[..bytes]);
            pixel
        });
        let at = (i / w) * pitch + (i % w) * bytes;
        out[at..at + bytes].copy_from_slice(&pixel[..bytes]);
    }
    Some(Made {
        buffer,
        width: picture.width,
        height: picture.height,
        pitch: pitch as u32,
        used: 0,
    })
}

fn copy(
    list: &ID3D12GraphicsCommandList,
    back: &ID3D12Resource,
    m: &Made,
    (x, y): (u32, u32),
    format: DXGI_FORMAT,
) {
    // SAFETY: an open list; the back buffer is a copy's destination in it and
    // the box lies inside it (checked by the caller); our buffer holds
    // `pitch * height` bytes and is kept for `KEEP` presents after this one.
    unsafe {
        let from = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(&m.buffer),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: 0,
                    Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: format,
                        Width: m.width,
                        Height: m.height,
                        Depth: 1,
                        RowPitch: m.pitch,
                    },
                },
            },
        };
        let to = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(back),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        list.CopyTextureRegion(&to, x, y, 0, &from, None);
    }
}

//! Cutscene skipping, for unattended runs and players alike: with
//! `LEONIDA_SKIP_SCENES=1` (off unless set, in every build), a few rows of the
//! presented picture are read back every [`EVERY`] frames from the world's
//! first frame on (the first that ran 500 draws or more), and while they show
//! a letterboxed picture Enter is held ([`hold_enter`], which the scripted
//! keys of `leonida-hle` add, and with them the stand-in pad's button for
//! Enter). Switched off, the present path checks the switch once a frame (read
//! from the environment once) and does nothing else.
//!
//! The title's opening is several cutscenes, each skipped by holding Enter
//! for a while; at 5 to 7 fps the fixed taps of `LEONIDA_KEYS` were too short
//! to skip any, and runs never left them. Everything here counts presented
//! frames, not seconds: the title's time moves with its frames.
//!
//! The rows are copied into a readback buffer by the present's own list and
//! judged at a later present, once that list has finished: nothing waits for
//! the card. No pixel leaves this module; the log says when a hold begins
//! and ends.

#[cfg(windows)]
use windows::Win32::Graphics::Direct3D12::*;
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::Common::*;

/// Every how many presented frames the picture is sampled.
const EVERY: u64 = 8;
/// Letterboxed samples in a row before Enter is held (a single one can be a
/// transition).
const LETTERBOX_SAMPLES: u32 = 2;
/// Open samples in a row before Enter is let go.
const OPEN_SAMPLES: u32 = 3;
/// While holding: frames Enter is down, then frames (and at least
/// [`RELEASE_MS`]) it is up, so a skip that wants a fresh press gets one.
/// A shorter release can fall inside one slow frame and never be seen.
/// How long a skip wants Enter down is not known, so every press of a hold
/// is twice as long as the one before, up to [`PRESS_MOST`]: one length cut
/// short every time would never skip.
const PRESS_FRAMES: u64 = 48;
const PRESS_MOST: u64 = 384;
const RELEASE_FRAMES: u64 = 3;
const RELEASE_MS: u64 = 500;
/// A hold that went this many frames without a black sample (the fade
/// between two cutscenes) stops: a letterbox Enter cannot skip. It is not
/// held again before an open or a black sample.
const CAP_FRAMES: u64 = 1200;

/// Rows read, as fractions of the height: three in the top bar, two in the
/// bottom bar, three in the picture. A cutscene's bars are 5.6% of the
/// height each (a 2:1 picture); subtitles reach down to 3% from the bottom,
/// so the bottom rows stay below that.
const ROWS: [f32; 8] = [0.01, 0.025, 0.04, 0.98, 0.99, 0.08, 0.5, 0.92];
/// The bar rows come first in [`ROWS`].
const BARS: usize = 5;
/// The brightest channel of a bar row, and the least of a picture row.
const BAR_LEVEL: u8 = 12;
const PICTURE_LEVEL: u8 = 24;

/// What a sample shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    /// Bars at the top and the bottom around a picture: a cutscene.
    Letterbox,
    /// No bar at the top.
    Open,
    /// Every row black: a fade or a loading screen.
    Black,
    /// None of these (a fade's edge): changes nothing.
    Unsure,
}

/// The row of a `height`-row picture at fraction `at`.
fn row_at(at: f32, height: u32) -> u32 {
    ((at * height as f32) as u32).min(height.saturating_sub(1))
}

/// The brightest color channel of every 4th pixel of an 8-bit four-channel
/// row (the fourth channel, alpha, is not looked at).
fn row_level(row: &[u8]) -> u8 {
    row.chunks_exact(4).step_by(4).flat_map(|p| p[..3].iter().copied()).max().unwrap_or(0)
}

/// What the levels of the [`ROWS`] say.
///
/// shortcut: a letterboxed picture is taken for a cutscene. A cutscene
/// without bars is not seen (the scripted taps are still pressed), and a
/// letterbox that is not one is held until [`CAP_FRAMES`].
fn classify(levels: &[u8; ROWS.len()]) -> Look {
    let bar = |level: &u8| *level <= BAR_LEVEL;
    if levels.iter().all(bar) {
        Look::Black
    } else if !bar(&levels[0]) || !bar(&levels[1]) {
        Look::Open
    } else if levels[..BARS].iter().all(bar) && levels[BARS..].iter().any(|&l| l > PICTURE_LEVEL) {
        Look::Letterbox
    } else {
        Look::Unsure
    }
}

/// A hold of Enter.
#[derive(Clone, Copy, Debug)]
struct Hold {
    /// When it began (frame, milliseconds).
    began: (u64, u64),
    /// The frame of its start or its last black sample, for [`CAP_FRAMES`].
    fresh: u64,
    /// When Enter last went down, and for how many frames.
    pressed: u64,
    press: u64,
    /// When Enter last went up, while it is up.
    released: Option<(u64, u64)>,
}

/// A change worth a log line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Event {
    Holding,
    /// Let go after this many seconds and frames.
    Released(f64, u64),
    Capped(f64, u64),
}

/// When to hold Enter, from samples and frames.
#[derive(Debug)]
pub(crate) struct Skipper {
    letterboxes: u32,
    opens: u32,
    hold: Option<Hold>,
    capped: bool,
}

impl Skipper {
    pub(crate) const fn new() -> Self {
        Self { letterboxes: 0, opens: 0, hold: None, capped: false }
    }

    /// A sample of frame `frame`, judged at `ms` milliseconds.
    pub(crate) fn look(&mut self, frame: u64, ms: u64, look: Look) -> Option<Event> {
        match look {
            Look::Letterbox => {
                self.letterboxes += 1;
                self.opens = 0;
                if self.hold.is_none() && !self.capped && self.letterboxes >= LETTERBOX_SAMPLES {
                    self.hold = Some(Hold {
                        began: (frame, ms),
                        fresh: frame,
                        pressed: frame,
                        press: PRESS_FRAMES,
                        released: None,
                    });
                    return Some(Event::Holding);
                }
            }
            Look::Open => {
                self.opens += 1;
                self.letterboxes = 0;
                self.capped = false;
                if self.opens >= OPEN_SAMPLES
                    && let Some(hold) = self.hold.take()
                {
                    return Some(Event::Released(
                        ms.saturating_sub(hold.began.1) as f64 / 1000.0,
                        frame.saturating_sub(hold.began.0),
                    ));
                }
            }
            // A hold carries across a fade between two cutscenes, and the
            // cutscene after one Enter could not skip is held again.
            Look::Black => {
                self.capped = false;
                if let Some(hold) = &mut self.hold {
                    hold.fresh = frame;
                }
            }
            Look::Unsure => {}
        }
        None
    }

    /// Whether Enter is down at the present of frame `frame`, at `ms`.
    pub(crate) fn enter(&mut self, frame: u64, ms: u64) -> (bool, Option<Event>) {
        let Some(hold) = &mut self.hold else {
            return (false, None);
        };
        if frame.saturating_sub(hold.fresh) >= CAP_FRAMES {
            let event = Event::Capped(
                ms.saturating_sub(hold.began.1) as f64 / 1000.0,
                frame.saturating_sub(hold.began.0),
            );
            self.hold = None;
            self.capped = true;
            return (false, Some(event));
        }
        let down = match hold.released {
            Some((at, at_ms)) => {
                let over = frame.saturating_sub(at) >= RELEASE_FRAMES
                    && ms.saturating_sub(at_ms) >= RELEASE_MS;
                if over {
                    hold.released = None;
                    hold.pressed = frame;
                    hold.press = (hold.press * 2).min(PRESS_MOST);
                }
                over
            }
            None if frame.saturating_sub(hold.pressed) >= hold.press => {
                hold.released = Some((frame, ms));
                false
            }
            None => true,
        };
        (down, None)
    }
}

#[cfg(windows)]
pub use run::hold_enter;
#[cfg(windows)]
pub(crate) use run::{landed, on, present, record, sent};

#[cfg(windows)]
mod run {
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;

    /// `LEONIDA_SKIP_SCENES=1` turns the skipping on (see the module); any
    /// other value or none leaves it off. Read once.
    pub(crate) fn on() -> bool {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("LEONIDA_SKIP_SCENES").is_some_and(|v| v == "1"))
    }

    static HOLD: AtomicBool = AtomicBool::new(false);

    /// Whether Enter is to be held now.
    pub fn hold_enter() -> bool {
        HOLD.load(Relaxed)
    }

    /// The readback buffer of the rows, made for one picture size and format.
    struct Rows {
        buffer: ID3D12Resource,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        /// Bytes from one row to the next (placement alignment).
        slot: u64,
    }

    // SAFETY: a D3D12 buffer (free-threaded), mapped only under `STATE`'s lock
    // once the copy into it has finished.
    unsafe impl Send for Rows {}

    struct State {
        drawn: u64,
        world: bool,
        next: u64,
        rows: Option<Rows>,
        /// A sample in the air: its frame and the submission it waits for.
        pending: Option<(u64, u64)>,
        skipper: Skipper,
        told: bool,
    }

    static STATE: Mutex<State> = Mutex::new(State {
        drawn: 0,
        world: false,
        next: 0,
        rows: None,
        pending: None,
        skipper: Skipper::new(),
        told: false,
    });

    fn state() -> MutexGuard<'static, State> {
        STATE.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tell(frame: u64, event: Event) {
        match event {
            Event::Holding => {
                tracing::info!(frame, "cutscene skip: holding Enter (a letterboxed picture)");
            }
            Event::Released(seconds, frames) => tracing::info!(
                frame,
                frames,
                "cutscene skip: Enter let go after {seconds:.1} s (an open picture)"
            ),
            Event::Capped(seconds, frames) => tracing::info!(
                frame,
                frames,
                "cutscene skip: Enter let go after {seconds:.1} s without a fade; \
                 not held again before an open picture"
            ),
        }
    }

    /// At the present of `frame`: the draws run on the host so far.
    pub(crate) fn present(frame: u64, drawn: u64) {
        if !on() {
            return;
        }
        let mut state = state();
        let before = std::mem::replace(&mut state.drawn, drawn);
        if !state.world && drawn.saturating_sub(before) >= 500 {
            state.world = true;
            state.next = frame;
            tracing::info!(
                frame,
                "cutscene skip: the world's first frame; its picture is sampled every {EVERY} \
                 frames (LEONIDA_SKIP_SCENES)"
            );
        }
        let (down, event) = state.skipper.enter(frame, crate::host::now_ms());
        if let Some(event) = event {
            tell(frame, event);
        }
        HOLD.store(down, Relaxed);
    }

    /// If a sample is due at the present of `frame`: records copies of the
    /// [`ROWS`] of `texture` (in the copy-source state) into `list`. The
    /// caller sends the list and says with [`sent`] which submission ran it.
    pub(crate) fn record(
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        texture: &ID3D12Resource,
        frame: u64,
    ) -> bool {
        if !on() {
            return false;
        }
        let mut state = state();
        if !state.world || state.pending.is_some() || frame < state.next {
            return false;
        }
        // SAFETY: a live resource.
        let d = unsafe { texture.GetDesc() };
        if !matches!(d.Format.0, 27..=29 | 87 | 90 | 91) {
            if !std::mem::replace(&mut state.told, true) {
                tracing::warn!(
                    format = d.Format.0,
                    "cutscene skip: the presented picture is not 8-bit color; nothing is skipped"
                );
            }
            return false;
        }
        let (width, height) = (d.Width as u32, d.Height);
        let size = (width, height, d.Format);
        if state.rows.as_ref().is_none_or(|r| (r.width, r.height, r.format) != size) {
            state.rows = made(device, width, height, d.Format);
        }
        let Some(rows) = &state.rows else {
            return false;
        };
        let pitch = (u64::from(width) * 4).next_multiple_of(256) as u32;
        for (i, &at) in ROWS.iter().enumerate() {
            let y = row_at(at, height);
            // SAFETY: an open list; locations naming live resources, the
            // buffer large enough for every row (`made`).
            unsafe {
                let to = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(&rows.buffer),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                            Offset: rows.slot * i as u64,
                            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                Format: d.Format,
                                Width: width,
                                Height: 1,
                                Depth: 1,
                                RowPitch: pitch,
                            },
                        },
                    },
                };
                let from = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(texture),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
                };
                let row =
                    D3D12_BOX { left: 0, top: y, front: 0, right: width, bottom: y + 1, back: 1 };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, Some(&row));
            }
        }
        state.pending = Some((frame, u64::MAX));
        state.next = frame + EVERY;
        true
    }

    /// A readback buffer for the rows of a `width`-wide picture.
    fn made(device: &ID3D12Device, width: u32, height: u32, format: DXGI_FORMAT) -> Option<Rows> {
        let slot = (u64::from(width) * 4).next_multiple_of(256).next_multiple_of(512);
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: slot * ROWS.len() as u64,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            ..Default::default()
        };
        let heap = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() };
        let mut buffer: Option<ID3D12Resource> = None;
        // SAFETY: plain creation.
        unsafe {
            device
                .CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    None,
                    &mut buffer,
                )
                .ok()?;
        }
        Some(Rows { buffer: buffer?, width, height, format, slot })
    }

    /// The sample [`record`] made went with submission `flown` (0: it ran
    /// and was waited for).
    pub(crate) fn sent(flown: u64) {
        if let Some((_, waits)) = &mut state().pending {
            *waits = flown;
        }
    }

    /// Judges the sample in the air if submissions up to `finished` have
    /// finished.
    pub(crate) fn landed(finished: u64) {
        if !on() {
            return;
        }
        let mut state = state();
        let Some((frame, waits)) = state.pending else {
            return;
        };
        if waits > finished {
            return;
        }
        state.pending = None;
        let Some(rows) = &state.rows else {
            return;
        };
        let mut levels = [0; ROWS.len()];
        let mut data = std::ptr::null_mut();
        // SAFETY: the copies finished (their submission has); the mapping
        // spans every row's slot.
        unsafe {
            if rows.buffer.Map(0, None, Some(&mut data)).is_err() {
                return;
            }
            let bytes = std::slice::from_raw_parts(
                data.cast::<u8>(),
                (rows.slot * ROWS.len() as u64) as usize,
            );
            for (i, level) in levels.iter_mut().enumerate() {
                let at = rows.slot as usize * i;
                *level = row_level(&bytes[at..at + rows.width as usize * 4]);
            }
            rows.buffer.Unmap(0, None);
        }
        let look = classify(&levels);
        if let Some(event) = state.skipper.look(frame, crate::host::now_ms(), look) {
            tell(frame, event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic picture's rows, `width` RGBA pixels each, of the given
    /// levels: the [`ROWS`] fractions picked from it as a run would.
    fn sample(height: u32, level_of: impl Fn(u32) -> u8) -> [u8; ROWS.len()] {
        let width = 64;
        let mut levels = [0; ROWS.len()];
        for (level, &at) in levels.iter_mut().zip(&ROWS) {
            let y = row_at(at, height);
            let row: Vec<u8> = (0..width)
                .flat_map(|x| {
                    // Brightness on one pixel in four only, and alpha opaque.
                    let v = if x % 4 == 0 { level_of(y) } else { 0 };
                    [v, v / 2, 0, 255]
                })
                .collect();
            *level = row_level(&row);
        }
        levels
    }

    /// A 2:1 picture inside bars of `bar` rows top and bottom (`height` rows).
    fn letterboxed(height: u32, bar: u32) -> [u8; ROWS.len()] {
        sample(height, |y| if y < bar || y >= height - bar { 3 } else { 120 })
    }

    #[test]
    fn pictures_are_told_apart() {
        for height in [1080, 2160, 720] {
            let bar = height * 60 / 1080;
            assert_eq!(classify(&letterboxed(height, bar)), Look::Letterbox, "{height}");
            assert_eq!(classify(&sample(height, |_| 90)), Look::Open);
            assert_eq!(classify(&sample(height, |_| 4)), Look::Black);
        }
        // Subtitles reaching into the bottom bar, and narrower bars.
        let subtitled = sample(1080, |y| if (60..1048).contains(&y) { 200 } else { 0 });
        assert_eq!(classify(&subtitled), Look::Letterbox);
        assert_eq!(classify(&letterboxed(1080, 48)), Look::Letterbox);
        // A dark picture between the bars, and thin bars of a transition.
        assert_eq!(
            classify(&sample(1080, |y| if (60..1020).contains(&y) { 20 } else { 0 })),
            Look::Unsure
        );
        assert_eq!(classify(&letterboxed(1080, 12)), Look::Open);
        // Only the top bar: not a letterbox.
        assert_eq!(classify(&sample(1080, |y| if y < 60 { 0 } else { 100 })), Look::Unsure);
    }

    #[test]
    fn alpha_and_skipped_pixels_are_not_read() {
        let row: Vec<u8> = (0..16)
            .flat_map(|x| if x % 4 == 0 { [5, 6, 7, 255] } else { [255; 4] })
            .collect();
        assert_eq!(row_level(&row), 7);
    }

    /// Runs `skipper` from frame `from` to `to`, sampling every [`EVERY`]
    /// frames with `look`, at 6 fps; the frames Enter was down.
    fn run(skipper: &mut Skipper, from: u64, to: u64, look: Look) -> (u64, Vec<Event>) {
        let mut down = 0;
        let mut events = Vec::new();
        for frame in from..to {
            let ms = frame * 1000 / 6;
            if frame.is_multiple_of(EVERY) {
                events.extend(skipper.look(frame, ms, look));
            }
            let (held, event) = skipper.enter(frame, ms);
            down += u64::from(held);
            events.extend(event);
        }
        (down, events)
    }

    #[test]
    fn a_cutscene_is_held_and_let_go() {
        let mut skipper = Skipper::new();
        // One letterboxed sample is not enough.
        let (down, events) = run(&mut skipper, 0, EVERY, Look::Letterbox);
        assert_eq!((down, events.len()), (0, 0));
        let (down, events) = run(&mut skipper, EVERY, 200, Look::Letterbox);
        assert_eq!(events, [Event::Holding]);
        // Down most of the time, up now and then.
        assert!(down > 150 && down < 192, "{down}");
        // A fade keeps the hold.
        let (down, events) = run(&mut skipper, 200, 240, Look::Black);
        assert!(down > 0 && events.is_empty());
        // Let go on the third open sample.
        let (_, events) = run(&mut skipper, 240, 300, Look::Open);
        assert!(matches!(events[..], [Event::Released(s, 248)] if s > 40.0), "{events:?}");
        assert_eq!(skipper.enter(301, 301_000 / 6), (false, None));
        // And held again for the next cutscene.
        let (_, events) = run(&mut skipper, 300, 340, Look::Letterbox);
        assert_eq!(events, [Event::Holding]);
    }

    #[test]
    fn a_release_lasts_frames_and_half_a_second() {
        let mut skipper = Skipper::new();
        skipper.look(0, 0, Look::Letterbox);
        skipper.look(8, 0, Look::Letterbox);
        assert!(skipper.enter(8, 0).0);
        assert!(!skipper.enter(8 + PRESS_FRAMES, 100).0);
        // Three frames on, but too soon (a fast frame rate).
        assert!(!skipper.enter(8 + PRESS_FRAMES + RELEASE_FRAMES, 300).0);
        assert!(skipper.enter(9 + PRESS_FRAMES + RELEASE_FRAMES, 600).0);
    }

    #[test]
    fn presses_grow_and_a_fade_ends_a_cap() {
        let mut skipper = Skipper::new();
        skipper.look(0, 0, Look::Letterbox);
        skipper.look(8, 0, Look::Letterbox);
        let mut presses = Vec::new();
        let mut length = 0;
        let mut capped = None;
        for frame in 8..8 + CAP_FRAMES + 1 {
            let (down, event) = skipper.enter(frame, frame * 1000 / 6);
            if down {
                length += 1;
            } else if length > 0 {
                presses.push(std::mem::take(&mut length));
            }
            capped = capped.or(event);
        }
        assert_eq!(presses[..5], [48, 96, 192, 384, 384]);
        assert!(matches!(capped, Some(Event::Capped(..))), "{capped:?}");
        // The next cutscene, after a fade, is held again.
        let end = 8 + CAP_FRAMES + 8;
        assert_eq!(skipper.look(end, 0, Look::Letterbox), None);
        skipper.look(end + 8, 0, Look::Black);
        assert_eq!(skipper.look(end + 16, 0, Look::Letterbox), Some(Event::Holding));
    }

    #[test]
    fn an_endless_letterbox_stops_being_held() {
        let mut skipper = Skipper::new();
        let (_, events) = run(&mut skipper, 0, 2 * CAP_FRAMES, Look::Letterbox);
        assert!(matches!(events[..], [Event::Holding, Event::Capped(..)]), "{events:?}");
        // Held again only after an open picture.
        let end = 2 * CAP_FRAMES;
        let (_, events) = run(&mut skipper, end, end + 16, Look::Open);
        assert!(events.is_empty());
        let (_, events) = run(&mut skipper, end + 16, end + 40, Look::Letterbox);
        assert_eq!(events, [Event::Holding]);
    }

    #[test]
    fn unsure_samples_change_nothing() {
        let mut skipper = Skipper::new();
        run(&mut skipper, 0, 24, Look::Letterbox);
        let (down, events) = run(&mut skipper, 24, 200, Look::Unsure);
        assert!(down > 0 && events.is_empty());
    }
}

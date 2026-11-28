//! The F3 performance panel, in the top left corner of the frame shown
//! (`crate::overlay`), in every build: the frame rate in large type, then the
//! frame time, the time spent preparing and waiting for the card's work, the
//! draws, the uploads, the card's memory, frame generation and the frame-rate
//! cap. The panel part of VIctory-NVIDIA 35c1560 was read as a reference.
//!
//! `LEONIDA_PANEL=1` shows it (off by default); live
//! (`leonida_core::switches`), so F3 and the F1 menu show and hide it at
//! once. While it shows, the small frame rate counter (`crate::fpsbox`) does
//! not: the panel has the number.

use std::sync::Mutex;

use crate::canvas::{self, AMBER, DIM, FLAMINGO, GLYPH_HEIGHT, PANEL, PAPER, Picture, Rgb, TEAL};

/// What the presenter's meter measured over the last second, a frame each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Stats {
    /// Frames the title rendered, a second.
    pub fps: f64,
    pub ms: f64,
    pub draws: f64,
    /// Preparing submissions, and waiting for the card.
    pub cpu_ms: f64,
    pub gpu_ms: f64,
    /// Copies, textures and buffers copied back.
    pub upload_mb: f64,
    pub dropped: f64,
    /// The card's own memory budget and use, then the PC's shared memory's
    /// (`Host::video_memory`), in bytes.
    pub memory: Option<[u64; 4]>,
}

static STATS: Mutex<Option<Stats>> = Mutex::new(None);

/// Whether the panel shows.
pub(crate) fn shown() -> bool {
    leonida_core::switches::live_flag("LEONIDA_PANEL")
}

/// The meter's numbers for the last second.
pub(crate) fn measured(stats: Stats) {
    *STATS.lock().unwrap_or_else(|e| e.into_inner()) = Some(stats);
}

/// Pixels a dot at a frame `height` lines high: 2 at 1080, 4 at 2160.
fn scale(height: u32) -> u32 {
    (height / 540).clamp(1, 6)
}

/// Pixels between the panel and the frame's corner.
pub(crate) fn margin(height: u32) -> u32 {
    4 * scale(height)
}

/// The panel's picture for a frame `height` lines high, when it shows.
pub(crate) fn picture(height: u32) -> Option<Picture> {
    if !shown() {
        return None;
    }
    let stats = *STATS.lock().unwrap_or_else(|e| e.into_inner());
    let fg = crate::overlay::framegen_running();
    let cap = leonida_core::switches::live_number("LEONIDA_FRAME_INTERVAL_US");
    let (big, lines) = rows(stats.as_ref(), fg, cap);
    Some(layout(&big, &lines, scale(height)))
}

/// The frame-rate cap as the menu names it.
fn cap_label(us: f64) -> String {
    let listed = leonida_core::switches::find("LEONIDA_FRAME_INTERVAL_US").and_then(|s| {
        s.values
            .iter()
            .find(|c| leonida_core::switches::number(c.value) == Some(us))
    });
    match listed {
        Some(c) => c.label.to_owned(),
        None if (1_000.0..=1_000_000.0).contains(&us) => format!("{:.0} FPS", 1e6 / us),
        None => "GAME'S OWN".to_owned(),
    }
}

type Line = (&'static str, String, Rgb);

/// The large number (frames shown a second) and the lines under it.
fn rows(stats: Option<&Stats>, fg: Option<(&str, u32)>, cap_us: f64) -> (String, Vec<Line>) {
    let per = fg.map_or(1, |(_, times)| times);
    let mut lines = Vec::new();
    let big = match stats {
        None => {
            lines.push(("MEASURING", String::new(), DIM));
            "--.-".to_owned()
        }
        Some(s) => {
            if per > 1 {
                lines.push(("RENDERED", format!("{:.1} FPS", s.fps), PAPER));
            }
            lines.push(("FRAME TIME", format!("{:.2} MS", s.ms), PAPER));
            lines.push((
                "CPU / GPU",
                format!("{:.1} / {:.1} MS", s.cpu_ms, s.gpu_ms),
                PAPER,
            ));
            lines.push(("DRAWS", format!("{:.0}", s.draws), PAPER));
            lines.push(("UPLOADS", format!("{:.1} MB", s.upload_mb), PAPER));
            if let Some([budget, used, _, shared]) = s.memory {
                let gb = |bytes: u64| bytes as f64 / f64::from(1u32 << 30);
                let tight = used > budget / 100 * 95;
                lines.push((
                    "CARD MEMORY",
                    format!("{:.1} / {:.1} GB", gb(used), gb(budget)),
                    if tight { AMBER } else { PAPER },
                ));
                if shared > 256 << 20 {
                    lines.push(("IN PC MEMORY", format!("{:.1} GB", gb(shared)), AMBER));
                }
            }
            if s.dropped > 0.0 {
                lines.push(("DROPPED DRAWS", format!("{:.1}", s.dropped), AMBER));
            }
            format!("{:.1}", s.fps * f64::from(per))
        }
    };
    lines.push(match fg {
        Some((kind, times)) => ("FRAME GENERATION", format!("{kind} {times}X"), TEAL),
        None => ("FRAME GENERATION", "OFF".to_owned(), PAPER),
    });
    lines.push(("FRAME-RATE CAP", cap_label(cap_us), PAPER));
    (big, lines)
}

/// Characters a line holds.
const COLUMNS: u32 = 34;
const PAD: u32 = 6;
const LINE: u32 = 11;

fn layout(big: &str, lines: &[Line], s: u32) -> Picture {
    let width = 2 * PAD + COLUMNS * canvas::ADVANCE - 1;
    let height =
        PAD + GLYPH_HEIGHT + 4 + 4 * GLYPH_HEIGHT + 6 + 1 + 4 + lines.len() as u32 * LINE - 4 + PAD;
    let mut p = Picture::new(width * s, height * s, PANEL);
    p.fill(0, 0, width * s, s, FLAMINGO);
    p.fill(0, 0, s, height * s, FLAMINGO);
    let right = width - PAD;
    let mut y = PAD;
    p.text(PAD * s, y * s, s, FLAMINGO, "LEONIDA");
    p.text(
        (right - canvas::text_width("F3", 1)) * s,
        y * s,
        s,
        DIM,
        "F3",
    );
    y += GLYPH_HEIGHT + 4;
    p.text(PAD * s, y * s, 4 * s, PAPER, big);
    let x = PAD + canvas::text_width(big, 4) + 6;
    p.text(x * s, (y + 2 * GLYPH_HEIGHT) * s, 2 * s, TEAL, "FPS");
    y += 4 * GLYPH_HEIGHT + 6;
    p.fill(PAD * s, y * s, (width - 2 * PAD) * s, s, FLAMINGO);
    y += 1 + 4;
    for (label, value, rgb) in lines {
        p.text(PAD * s, y * s, s, DIM, label);
        let left = right.saturating_sub(canvas::text_width(value, 1));
        p.text(left * s, y * s, s, *rgb, value);
        y += LINE;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> Stats {
        Stats {
            fps: 59.94,
            ms: 16.68,
            draws: 1234.4,
            cpu_ms: 3.12,
            gpu_ms: 5.2,
            upload_mb: 12.34,
            dropped: 0.0,
            memory: Some([12 << 30, 6 << 30, 16 << 30, 0]),
        }
    }

    #[test]
    fn the_lines_say_what_was_measured() {
        let (big, lines) = rows(Some(&stats()), None, 0.0);
        assert_eq!(big, "59.9");
        let text: Vec<String> = lines.iter().map(|(l, v, _)| format!("{l} {v}")).collect();
        assert_eq!(
            text,
            [
                "FRAME TIME 16.68 MS",
                "CPU / GPU 3.1 / 5.2 MS",
                "DRAWS 1234",
                "UPLOADS 12.3 MB",
                "CARD MEMORY 6.0 / 12.0 GB",
                "FRAME GENERATION OFF",
                "FRAME-RATE CAP GAME'S OWN",
            ]
        );
        assert!(lines.iter().all(|(_, _, rgb)| *rgb != AMBER));
    }

    #[test]
    fn frame_generation_multiplies_the_large_number_and_warnings_are_amber() {
        let tight = Stats {
            memory: Some([10 << 30, 9_900 << 20, 16 << 30, 1 << 30]),
            dropped: 2.5,
            ..stats()
        };
        let (big, lines) = rows(Some(&tight), Some(("DLSS", 3)), 8_333.0);
        assert_eq!(big, "179.8");
        let get = |name: &str| lines.iter().find(|(l, _, _)| *l == name).unwrap().clone();
        assert_eq!(get("RENDERED").1, "59.9 FPS");
        assert_eq!(get("CARD MEMORY").2, AMBER);
        assert_eq!(
            get("IN PC MEMORY"),
            ("IN PC MEMORY", "1.0 GB".to_owned(), AMBER)
        );
        assert_eq!(get("DROPPED DRAWS").2, AMBER);
        assert_eq!(get("FRAME GENERATION").1, "DLSS 3X");
        assert_eq!(get("FRAME-RATE CAP").1, "120 FPS");
        // Before the first second: no numbers yet.
        let (big, lines) = rows(None, None, 0.0);
        assert_eq!((big.as_str(), lines[0].0), ("--.-", "MEASURING"));
    }

    #[test]
    fn caps_are_named_like_the_menu_and_others_by_their_rate() {
        assert_eq!(cap_label(16_667.0), "60 FPS");
        assert_eq!(cap_label(1_000.0), "NONE");
        assert_eq!(cap_label(20_000.0), "50 FPS");
        assert_eq!(cap_label(0.0), "GAME'S OWN");
        assert_eq!(cap_label(5.0), "GAME'S OWN");
    }

    #[test]
    fn the_panel_holds_its_lines_and_the_large_number() {
        let (big, lines) = rows(Some(&stats()), Some(("FSR 3", 2)), 0.0);
        let p = layout(&big, &lines, 2);
        assert_eq!(p.width, (2 * PAD + COLUMNS * canvas::ADVANCE - 1) * 2);
        for op in &p.ops {
            if let canvas::Op::Text {
                x, y, text, scale, ..
            } = op
            {
                assert!(x + canvas::text_width(text, *scale) <= p.width, "{text}");
                assert!(y + GLYPH_HEIGHT * scale <= p.height, "{text}");
            }
        }
        // Even a rate of thousands with frame generation fits.
        let (big, lines) = rows(
            Some(&Stats {
                fps: 999.9,
                ..stats()
            }),
            Some(("DLSS", 6)),
            0.0,
        );
        let p = layout(&big, &lines, 1);
        let fps = p.ops.iter().find_map(|op| match op {
            canvas::Op::Text { x, text, .. } if text == "FPS" => Some(*x),
            _ => None,
        });
        assert!(fps.unwrap() + canvas::text_width("FPS", 2) <= p.width);
    }
}

//! The F1 settings menu, drawn into the frame shown (`crate::overlay`) in
//! Leonida's look. VIctory-NVIDIA's menu (6395727, 22c6be2, 86a3976, 5986a64)
//! was read as a reference; this one is our own.
//!
//! Four tabs (DISPLAY, FRAME GENERATION, PICTURE, CONTROLS) list the live
//! switches of their group in `leonida_core::switches`, each with its value,
//! and one line of help for the selected one at the bottom. Q and E (or a
//! click) change tab, Up and Down (W, S) move, Left and Right (A, D) change
//! the value, Enter or a click moves it on, and Esc or F1 closes. F3 shows
//! and hides the panel (`crate::panel`) whether the menu is open or not.
//! A change applies at once and goes into the shared settings file after
//! 1.5 quiet seconds, and when the menu closes at the latest. Switches that
//! need a restart are not here: the last row says they are in the launcher.
//!
//! Keys and clicks come from `leonida_hle`'s input poll ([`input`],
//! [`click`]), which also keeps them from the game while the menu is open
//! ([`crate::SETTINGS_MENU_OPEN`] keeps the mouse from it).
//! `LEONIDA_MENU=0` (read once) takes the menu and the F3 key away, so both
//! keys go to the game; `LEONIDA_MENU_OPEN=1` opens the menu at the start.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use leonida_core::switches::{self, Group, Switch};

use crate::canvas::{
    self, ADVANCE, AMBER, DIM, FLAMINGO, GLYPH_HEIGHT, NEON, PANEL, PAPER, Picture, TEAL,
};

/// The tabs, in order.
const TABS: [Group; 4] = [
    Group::Display,
    Group::FrameGeneration,
    Group::Picture,
    Group::Controls,
];

/// The last row of every tab.
pub const LAUNCHER: &str = "More settings (need a restart) are in the Leonida launcher";
const KEYS: &str = "Q E TAB   W S MOVE   A D CHANGE   ENTER TOGGLE   ESC CLOSE";
/// How long after the last change the settings file is written.
const QUIET: Duration = Duration::from_millis(1500);

/// The virtual-key codes the menu answers to, besides the letters.
pub mod key {
    pub const F1: u8 = 0x70;
    pub const F3: u8 = 0x72;
    pub const ESC: u8 = 0x1B;
    pub const ENTER: u8 = 0x0D;
    pub const LEFT: u8 = 0x25;
    pub const UP: u8 = 0x26;
    pub const RIGHT: u8 = 0x27;
    pub const DOWN: u8 = 0x28;
}

/// Whether the menu and the F3 key are there (`LEONIDA_MENU`, on unless 0).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("LEONIDA_MENU")
            .ok()
            .as_deref()
            .and_then(switches::number)
            != Some(0.0)
    })
}

/// Where the menu is: open or not, the tab, the selected row of the tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Nav {
    open: bool,
    tab: usize,
    row: usize,
}

struct State {
    nav: Nav,
    /// Where the last picture put the tabs and rows, for clicks.
    hits: Option<Hits>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> MutexGuard<'static, Option<State>> {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.is_none() {
        let open = enabled() && std::env::var_os("LEONIDA_MENU_OPEN").is_some_and(|v| v == "1");
        if open {
            tracing::info!("settings menu open from the start (LEONIDA_MENU_OPEN)");
        }
        crate::SETTINGS_MENU_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);
        *state = Some(State {
            nav: Nav {
                open,
                ..Nav::default()
            },
            hits: None,
        });
    }
    state
}

/// Whether the menu is open: the game then gets no keys and no mouse.
pub fn is_open() -> bool {
    state().as_ref().is_some_and(|s| s.nav.open)
}

/// How a value changes: a step along its values (stopping at the ends), or
/// on to the next (from the last back to the first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum How {
    Step(i32),
    Cycle,
}

/// What a key asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    Nothing,
    Opened,
    Closed,
    Panel,
    Change(How),
}

/// Moves `nav` for a key pressed (`vk`); `rows` gives the selectable rows of
/// a tab.
fn step(nav: &mut Nav, vk: u8, rows: impl Fn(usize) -> usize) -> Do {
    if vk == key::F3 {
        return Do::Panel;
    }
    if !nav.open {
        if vk == key::F1 {
            nav.open = true;
            return Do::Opened;
        }
        return Do::Nothing;
    }
    let n = rows(nav.tab);
    nav.row = nav.row.min(n.saturating_sub(1));
    match vk {
        key::F1 | key::ESC => {
            nav.open = false;
            Do::Closed
        }
        b'Q' | b'E' => {
            let by = if vk == b'Q' { TABS.len() - 1 } else { 1 };
            nav.tab = (nav.tab + by) % TABS.len();
            nav.row = 0;
            Do::Nothing
        }
        key::UP | b'W' if n > 0 => {
            nav.row = (nav.row + n - 1) % n;
            Do::Nothing
        }
        key::DOWN | b'S' if n > 0 => {
            nav.row = (nav.row + 1) % n;
            Do::Nothing
        }
        key::LEFT | b'A' if n > 0 => Do::Change(How::Step(-1)),
        key::RIGHT | b'D' if n > 0 => Do::Change(How::Step(1)),
        key::ENTER if n > 0 => Do::Change(How::Cycle),
        _ => Do::Nothing,
    }
}

/// The live switches of a group, as the menu lists them.
fn rows(group: Group) -> Vec<&'static Switch> {
    switches::SWITCHES
        .iter()
        .filter(|s| s.group == group && s.live)
        .collect()
}

/// The place in `s.values` of the value nearest `current`.
fn nearest(s: &Switch, current: f64) -> usize {
    let distance =
        |value: &str| switches::number(value).map_or(f64::INFINITY, |v| (v - current).abs());
    (0..s.values.len())
        .min_by(|&a, &b| distance(s.values[a].value).total_cmp(&distance(s.values[b].value)))
        .unwrap_or(0)
}

/// The value `s` takes when changed `how` from `current`.
fn changed(s: &Switch, current: f64, how: How) -> &'static str {
    let n = s.values.len();
    let i = nearest(s, current);
    let next = match how {
        How::Step(by) => (i as i64 + i64::from(by)).clamp(0, n as i64 - 1) as usize,
        How::Cycle => (i + 1) % n,
    };
    s.values[next].value
}

/// The value as the menu shows it, and whether it is on (drawn in teal).
fn shown(s: &Switch, current: f64) -> (String, bool) {
    match s
        .values
        .iter()
        .find(|c| switches::number(c.value) == Some(current))
    {
        Some(c) => (c.label.to_owned(), c.label == "ON"),
        None => (format!("{current}"), false),
    }
}

/// A row's value as the menu shows it, and whether it is on.
fn row_value(s: &Switch) -> (String, bool) {
    shown(s, switches::live_number(s.name))
}

/// A row of a tab.
#[derive(Clone, Debug, PartialEq)]
enum Line {
    /// A live switch: selectable, its value changes.
    Switch {
        label: String,
        value: String,
        on: bool,
    },
    /// Something the menu shows but does not change.
    Info { label: String, value: String },
    /// A line of text.
    Note(String),
}

fn lines(tab: usize) -> Vec<Line> {
    let mut lines: Vec<Line> = rows(TABS[tab])
        .into_iter()
        .map(|s| {
            let (value, on) = row_value(s);
            Line::Switch {
                label: s.label.to_owned(),
                value,
                on,
            }
        })
        .collect();
    if TABS[tab] == Group::FrameGeneration {
        lines.push(Line::Info {
            label: "FRAME GENERATION TYPE".to_owned(),
            value: framegen_type(),
        });
    }
    if lines.is_empty() {
        lines.push(Line::Note(
            "Nothing here changes while the game runs yet".to_owned(),
        ));
    }
    lines.push(Line::Note(LAUNCHER.to_owned()));
    lines
}

/// The frame generation that runs, or the one asked for at the start.
fn framegen_type() -> String {
    let asked = |name: &str| {
        std::env::var(name)
            .ok()
            .as_deref()
            .and_then(switches::number)
            .is_some_and(|v| v != 0.0)
    };
    match crate::overlay::framegen_running() {
        Some((kind, _)) => kind.to_owned(),
        None if asked("LEONIDA_FRAMEGEN") => "DLSS (IDLE)".to_owned(),
        None if asked("LEONIDA_FSR_FRAMEGEN") => "FSR 3 (IDLE)".to_owned(),
        None => "OFF".to_owned(),
    }
}

/// Where the tabs and the selectable rows are, in pixels of the picture, and
/// where the picture is in the frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct Hits {
    at: (u32, u32),
    frame: (u32, u32),
    size: (u32, u32),
    /// Left, top, right, bottom.
    tabs: Vec<[u32; 4]>,
    /// Top and bottom.
    rows: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Tab(usize),
    Row(usize),
}

impl Hits {
    /// What lies at `fx`, `fy` (fractions of the window's width and height).
    fn find(&self, fx: f32, fy: f32) -> Option<Hit> {
        if !(0.0..1.0).contains(&fx) || !(0.0..1.0).contains(&fy) {
            return None;
        }
        let x = (fx * self.frame.0 as f32) as i64 - i64::from(self.at.0);
        let y = (fy * self.frame.1 as f32) as i64 - i64::from(self.at.1);
        if x < 0 || y < 0 || x >= i64::from(self.size.0) || y >= i64::from(self.size.1) {
            return None;
        }
        let (x, y) = (x as u32, y as u32);
        if let Some(i) = self
            .tabs
            .iter()
            .position(|&[x0, y0, x1, y1]| (x0..x1).contains(&x) && (y0..y1).contains(&y))
        {
            return Some(Hit::Tab(i));
        }
        self.rows
            .iter()
            .position(|&(y0, y1)| (y0..y1).contains(&y))
            .map(Hit::Row)
    }
}

/// Characters a row holds.
const COLUMNS: u32 = 64;
/// Dots of room around everything.
const PAD: u32 = 8;
/// Dots a row takes down.
const ROW: u32 = 13;
/// Dots a line of help takes down, and lines of it at most.
const HELP_LINE: u32 = 11;
const HELP_LINES: u32 = 3;

/// The menu's picture for a frame of `frame` pixels, and where its parts are.
/// Sizes are in dots of the font, drawn at a whole number of pixels each (2 at
/// 1080 lines, 4 at 2160); `room` rows are kept so the box does not change
/// size between tabs. None when the frame is too small.
fn layout(
    nav: &Nav,
    lines: &[Line],
    room: usize,
    help: &str,
    frame: (u32, u32),
) -> Option<(Picture, Hits)> {
    let width = 2 * PAD + COLUMNS * ADVANCE - 1;
    let body = room.max(lines.len()) as u32 * ROW;
    let height = PAD
        + 2 * GLYPH_HEIGHT
        + 6
        + ROW
        + 4
        + 1
        + 4
        + body
        + 4
        + 1
        + 4
        + HELP_LINES * HELP_LINE
        + 2
        + GLYPH_HEIGHT
        + PAD;
    let mut s = (frame.1 / 480).clamp(1, 6);
    while s > 0 && (width * s > frame.0 || height * s > frame.1) {
        s -= 1;
    }
    if s == 0 {
        return None;
    }
    let mut p = Picture::new(width * s, height * s, PANEL);
    let at = (
        (frame.0 - p.width) / 2,
        (frame.1 / 10).min(frame.1 - p.height),
    );
    let mut hits = Hits {
        at,
        frame,
        size: (p.width, p.height),
        ..Hits::default()
    };
    // The frame.
    p.fill(0, 0, width * s, s, FLAMINGO);
    p.fill(0, (height - 1) * s, width * s, s, FLAMINGO);
    p.fill(0, 0, s, height * s, FLAMINGO);
    p.fill((width - 1) * s, 0, s, height * s, FLAMINGO);

    let mut y = PAD;
    p.text(PAD * s, y * s, 2 * s, FLAMINGO, "LEONIDA");
    let name = canvas::text_width("LEONIDA", 2);
    p.text(
        (PAD + name + 8) * s,
        (y + GLYPH_HEIGHT) * s,
        s,
        PAPER,
        "SETTINGS",
    );
    y += 2 * GLYPH_HEIGHT + 6;

    let mut x = PAD;
    for (i, group) in TABS.iter().enumerate() {
        let label = group.label();
        let w = canvas::text_width(label, 1) + 6;
        let selected = i == nav.tab;
        if selected {
            p.fill(x * s, y * s, w * s, ROW * s, NEON);
        }
        let rgb = if selected { PANEL } else { DIM };
        p.text((x + 3) * s, (y + 3) * s, s, rgb, label);
        hits.tabs.push([x * s, y * s, (x + w) * s, (y + ROW) * s]);
        x += w + 4;
    }
    y += ROW + 4;
    p.fill(PAD * s, y * s, (width - 2 * PAD) * s, s, FLAMINGO);
    y += 1 + 4;

    let top = y;
    let right = width - PAD - 4;
    let mut row = 0;
    for line in lines {
        let ty = (y + 3) * s;
        match line {
            Line::Switch { label, value, on } => {
                let selected = row == nav.row;
                if selected {
                    p.fill(PAD * s, y * s, (width - 2 * PAD) * s, ROW * s, FLAMINGO);
                }
                let value = if selected {
                    format!("< {value} >")
                } else {
                    value.clone()
                };
                let rgb = if *on { TEAL } else { PAPER };
                p.text((PAD + 4) * s, ty, s, PAPER, label);
                let left = right.saturating_sub(canvas::text_width(&value, 1));
                p.text(left * s, ty, s, rgb, &value);
                hits.rows.push((y * s, (y + ROW) * s));
                row += 1;
            }
            Line::Info { label, value } => {
                p.text((PAD + 4) * s, ty, s, DIM, label);
                let left = right.saturating_sub(canvas::text_width(value, 1));
                p.text(left * s, ty, s, DIM, value);
            }
            Line::Note(text) => p.text((PAD + 4) * s, ty, s, AMBER, text),
        }
        y += ROW;
    }
    y = top + body + 4;
    p.fill(PAD * s, y * s, (width - 2 * PAD) * s, s, DIM);
    y += 1 + 4;
    for (k, line) in canvas::wrap(help, (COLUMNS - 2) as usize)
        .iter()
        .take(HELP_LINES as usize)
        .enumerate()
    {
        p.text(
            (PAD + 4) * s,
            (y + k as u32 * HELP_LINE) * s,
            s,
            PAPER,
            line,
        );
    }
    y += HELP_LINES * HELP_LINE + 2;
    p.text((PAD + 4) * s, y * s, s, DIM, KEYS);
    Some((p, hits))
}

/// The menu's picture for a frame of `frame` pixels and where it goes, when
/// the menu is open. Notes where its parts are, for [`click`].
pub(crate) fn picture(frame: (u32, u32)) -> Option<(u32, u32, Picture)> {
    let mut guard = state();
    let state = guard.as_mut()?;
    if !state.nav.open {
        state.hits = None;
        return None;
    }
    let room = (0..TABS.len()).map(|t| lines(t).len()).max().unwrap_or(0);
    let rows = rows(TABS[state.nav.tab]);
    state.nav.row = state.nav.row.min(rows.len().saturating_sub(1));
    let help = rows.get(state.nav.row).map_or("", |s| s.help);
    let (picture, hits) = layout(&state.nav, &lines(state.nav.tab), room, help, frame)?;
    let at = hits.at;
    state.hits = Some(hits);
    Some((at.0, at.1, picture))
}

/// Keys pressed since the last poll (virtual-key codes), in order. Returns
/// whether the menu is open after them.
pub fn input(pressed: &[u8]) -> bool {
    if !enabled() {
        return false;
    }
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return false;
    };
    for &vk in pressed {
        let count = |tab: usize| rows(TABS[tab]).len();
        match step(&mut state.nav, vk, count) {
            Do::Nothing => {}
            Do::Opened => tracing::info!("settings menu opened (F1)"),
            Do::Closed => {
                tracing::info!("settings menu closed");
                state.hits = None;
                save_aside(switches::save_now);
            }
            Do::Panel => {
                let on = switches::live_flag("LEONIDA_PANEL");
                switches::set("LEONIDA_PANEL", if on { "0" } else { "1" });
                tracing::info!(shown = !on, "performance panel (F3)");
            }
            Do::Change(how) => {
                if let Some(s) = rows(TABS[state.nav.tab]).get(state.nav.row) {
                    change(s, how);
                }
            }
        }
    }
    crate::SETTINGS_MENU_OPEN.store(state.nav.open, std::sync::atomic::Ordering::Relaxed);
    state.nav.open
}

/// A left click at `fx`, `fy` (fractions of the window's width and height)
/// while the menu is open: a tab is chosen, a row selected and moved on.
pub fn click(fx: f32, fy: f32) {
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return;
    };
    if !state.nav.open {
        return;
    }
    match state.hits.as_ref().and_then(|h| h.find(fx, fy)) {
        Some(Hit::Tab(tab)) => {
            state.nav.tab = tab;
            state.nav.row = 0;
        }
        Some(Hit::Row(row)) => {
            state.nav.row = row;
            if let Some(s) = rows(TABS[state.nav.tab]).get(row) {
                change(s, How::Cycle);
            }
        }
        None => {}
    }
}

fn change(s: &Switch, how: How) {
    let value = changed(s, switches::live_number(s.name), how);
    if !switches::set(s.name, value) {
        return;
    }
    tracing::info!(switch = s.name, value, "settings menu");
    // VIctory-NVIDIA's frame generation reads what is asked from its own
    // atomic, from the next present on; without frame generation it waits.
    if s.name == "LEONIDA_FRAMEGEN_X" {
        crate::streamline::set_framegen(value.parse().unwrap_or(0));
    }
}

/// At every frame shown: the settings file is written once changes have been
/// quiet for a while.
pub(crate) fn save_when_quiet() {
    if switches::save_is_due(QUIET) {
        save_aside(|| switches::save_due(QUIET));
    }
}

/// A save runs on its thread.
static SAVING: AtomicBool = AtomicBool::new(false);

/// Runs `save` on a thread of its own, one at a time: waiting for the
/// launcher's lock on the file, or for a program that holds it open, must not
/// hold up a frame or the menu's keys. A change it misses is saved once quiet.
fn save_aside<E: std::fmt::Display>(
    save: impl FnOnce() -> Option<Result<PathBuf, E>> + Send + 'static,
) {
    if SAVING.swap(true, Ordering::Acquire) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("leonida-settings-save".into())
        .spawn(move || {
            saved(save());
            SAVING.store(false, Ordering::Release);
        });
    if let Err(e) = spawned {
        SAVING.store(false, Ordering::Release);
        tracing::warn!(error = %e, "settings not saved: no thread to write them");
    }
}

fn saved<E: std::fmt::Display>(result: Option<Result<PathBuf, E>>) {
    match result {
        Some(Ok(path)) => tracing::info!(path = %path.display(), "settings saved"),
        Some(Err(e)) => tracing::warn!(error = %format!("{e:#}"), "settings not saved"),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(name: &str) -> &'static Switch {
        switches::find(name).unwrap()
    }

    #[test]
    fn each_tab_lists_only_its_live_switches() {
        let names = |g| rows(g).iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(
            names(Group::Display),
            [
                "LEONIDA_FPS_COUNTER",
                "LEONIDA_PANEL",
                "LEONIDA_FRAME_INTERVAL_US"
            ]
        );
        assert_eq!(names(Group::FrameGeneration), ["LEONIDA_FRAMEGEN_X"]);
        assert!(names(Group::Picture).is_empty());
        assert_eq!(names(Group::Controls).len(), 4);
        assert!(TABS.iter().all(|&g| rows(g).iter().all(|s| s.live)));
        // Every live switch's help fits the help lines.
        for s in switches::SWITCHES.iter().filter(|s| s.live) {
            let lines = canvas::wrap(s.help, (COLUMNS - 2) as usize).len();
            assert!(lines <= HELP_LINES as usize, "{}", s.name);
        }
        // The last row of every tab sends restart settings to the launcher.
        for tab in 0..TABS.len() {
            assert_eq!(lines(tab).last(), Some(&Line::Note(LAUNCHER.to_owned())));
        }
    }

    #[test]
    fn keys_move_through_tabs_and_rows() {
        let rows = |tab: usize| [3, 1, 0, 4][tab];
        let mut nav = Nav::default();
        assert_eq!(
            step(&mut nav, b'S', rows),
            Do::Nothing,
            "closed: keys are the game's"
        );
        assert_eq!(nav, Nav::default());
        assert_eq!(step(&mut nav, key::F3, rows), Do::Panel);
        assert_eq!(step(&mut nav, key::F1, rows), Do::Opened);
        assert_eq!(step(&mut nav, key::UP, rows), Do::Nothing);
        assert_eq!(nav.row, 2, "up from the first row wraps");
        step(&mut nav, b'S', rows);
        assert_eq!(nav.row, 0);
        assert_eq!(step(&mut nav, key::RIGHT, rows), Do::Change(How::Step(1)));
        assert_eq!(step(&mut nav, b'A', rows), Do::Change(How::Step(-1)));
        assert_eq!(step(&mut nav, key::ENTER, rows), Do::Change(How::Cycle));
        step(&mut nav, b'Q', rows);
        assert_eq!((nav.tab, nav.row), (3, 0), "Q from the first tab wraps");
        step(&mut nav, b'E', rows);
        step(&mut nav, b'E', rows);
        step(&mut nav, b'E', rows);
        assert_eq!(nav.tab, 2);
        assert_eq!(
            step(&mut nav, key::ENTER, rows),
            Do::Nothing,
            "a tab without rows"
        );
        assert_eq!(step(&mut nav, key::ESC, rows), Do::Closed);
        assert!(!nav.open);
        step(&mut nav, key::F1, rows);
        assert_eq!(step(&mut nav, key::F1, rows), Do::Closed, "F1 closes too");
    }

    #[test]
    fn values_step_to_the_ends_and_cycle_round() {
        let flag = find("LEONIDA_KBM_GTA");
        assert_eq!(changed(flag, 0.0, How::Cycle), "1");
        assert_eq!(changed(flag, 1.0, How::Cycle), "0");
        assert_eq!(changed(flag, 1.0, How::Step(1)), "1");
        let sens = find("LEONIDA_MOUSE_SENS");
        assert_eq!(changed(sens, 1.0, How::Step(1)), "1.25");
        assert_eq!(changed(sens, 0.25, How::Step(-1)), "0.25");
        assert_eq!(changed(sens, 3.0, How::Step(1)), "3");
        // A value set by hand that is not listed: from the nearest one.
        assert_eq!(changed(sens, 1.1, How::Step(1)), "1.25");
        assert_eq!(shown(sens, 1.1), ("1.1".to_owned(), false));
        assert_eq!(shown(sens, 2.0), ("2.0".to_owned(), false));
        assert_eq!(shown(flag, 1.0), ("ON".to_owned(), true));
        let cap = find("LEONIDA_FRAME_INTERVAL_US");
        assert_eq!(shown(cap, 0.0).0, "GAME'S OWN");
        assert_eq!(changed(cap, 0.0, How::Step(1)), "33333");
    }

    fn sample() -> Vec<Line> {
        let switch = |label: &str, value: &str| Line::Switch {
            label: label.to_owned(),
            value: value.to_owned(),
            on: value == "ON",
        };
        vec![
            switch("FPS ON SCREEN", "ON"),
            switch("PERFORMANCE PANEL", "OFF"),
            switch("FRAME-RATE CAP", "GAME'S OWN"),
            Line::Note(LAUNCHER.to_owned()),
        ]
    }

    #[test]
    fn the_box_fits_the_frame_and_clicks_find_tabs_and_rows() {
        let nav = Nav {
            open: true,
            tab: 0,
            row: 1,
        };
        let (p, hits) = layout(&nav, &sample(), 6, "Shows the frame rate.", (1920, 1080)).unwrap();
        // Two pixels a dot at 1080 lines, in the middle, a tenth down.
        assert_eq!(p.width, (2 * PAD + COLUMNS * ADVANCE - 1) * 2);
        assert_eq!(hits.at, ((1920 - p.width) / 2, 108));
        assert!(hits.at.1 + p.height <= 1080);
        assert_eq!((hits.tabs.len(), hits.rows.len()), (4, 3));
        let centre = |[x0, y0, x1, y1]: [u32; 4]| {
            (
                (hits.at.0 + (x0 + x1) / 2) as f32 / 1920.0,
                (hits.at.1 + (y0 + y1) / 2) as f32 / 1080.0,
            )
        };
        let (fx, fy) = centre(hits.tabs[3]);
        assert_eq!(hits.find(fx, fy), Some(Hit::Tab(3)));
        let (y0, y1) = hits.rows[2];
        let (fx, fy) = centre([0, y0, p.width, y1]);
        assert_eq!(hits.find(fx, fy), Some(Hit::Row(2)));
        assert_eq!(hits.find(0.01, 0.01), None, "outside the box");
        assert_eq!(hits.find(-0.5, 2.0), None);
        // The selected row (1) is flamingo behind its text, with arrows round
        // the value; the others are not.
        let arrows =
            |op: &canvas::Op| matches!(op, canvas::Op::Text { text, .. } if text == "< OFF >");
        assert!(p.ops.iter().any(arrows));
        let px = p.render();
        let at = |row: usize| {
            let (x, y) = ((PAD + 1) * 2, hits.rows[row].0 + 1);
            px[(y * p.width + x) as usize]
        };
        assert_eq!((at(1), at(2)), (FLAMINGO, PANEL));
        // The same rows at 2160 lines: four pixels a dot.
        let (big, _) = layout(&nav, &sample(), 6, "", (3840, 2160)).unwrap();
        assert_eq!(big.width, p.width * 2);
        // Too small a frame: nothing.
        assert!(layout(&nav, &sample(), 6, "", (320, 200)).is_none());
    }

    #[test]
    fn the_box_keeps_its_size_between_tabs_and_wraps_long_help() {
        let nav = Nav {
            open: true,
            ..Nav::default()
        };
        let long = "word ".repeat(60);
        let (a, _) = layout(&nav, &sample(), 6, &long, (1920, 1080)).unwrap();
        let (b, _) = layout(&nav, &sample()[3..], 6, "", (1920, 1080)).unwrap();
        assert_eq!((a.width, a.height), (b.width, b.height));
        let help_lines = a
            .ops
            .iter()
            .filter(|op| matches!(op, canvas::Op::Text { text, .. } if text.starts_with("word")))
            .count();
        assert_eq!(help_lines, HELP_LINES as usize);
        // Every text fits inside the frame of the box.
        for op in &a.ops {
            if let canvas::Op::Text { x, text, scale, .. } = op {
                assert!(
                    x + canvas::text_width(text, *scale) <= a.width - 2,
                    "{text}"
                );
            }
        }
    }
}

//! Every frame's length, read every five seconds.
//!
//! The frame-rate line averages over a second or more, so a single long
//! frame is lost in it. At the end of each present call the time since the
//! present before is that frame's length. Every five seconds one line gives
//! the frame times (the shortest, the median, the longest and each one) and
//! one gives the usual frame (the mean of the frames that were not long),
//! the two lines `scripts\measure-run.ps1` sums up.
//!
//! shortcut: the lengths only. Leonida's own frame log also gives what
//! each frame was made of, from counters this renderer does not keep.
//!
//! On by default in diagnostics builds, off in others;
//! `LEONIDA_FRAME_LOG=1` or `0` says otherwise.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Whether frames are logged.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| match std::env::var_os("LEONIDA_FRAME_LOG") {
        Some(v) => v != "0",
        None => cfg!(feature = "diagnostics"),
    })
}

struct State {
    last: Option<Instant>,
    since: Instant,
    /// This reading's frame lengths, in microseconds.
    frames: Vec<f64>,
    /// The lengths of the last frames, for the usual one.
    recent: VecDeque<f64>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Frames whose lengths give the usual one.
const RECENT: usize = 64;

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut v: Vec<f64> = values.collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Over how many microseconds a frame is long, the usual one being
/// `usual`: half as long again, and at least 40 ms more.
fn long_over(usual: f64) -> f64 {
    (usual * 1.5).max(usual + 40_000.0)
}

/// The lines of a reading: the frame times and the usual frame.
fn lines(frames: &[f64], usual: f64, seconds: f64) -> Vec<String> {
    let over = long_over(usual);
    let (long, plain): (Vec<f64>, Vec<f64>) = frames.iter().partition(|&&us| us > over);
    let each: Vec<String> = frames.iter().map(|us| format!("{:.0}", us / 1e3)).collect();
    let shortest = frames.iter().copied().fold(f64::INFINITY, f64::min);
    let longest = frames.iter().copied().fold(0.0, f64::max);
    let mut out = vec![format!(
        "frame times: {} frames in {seconds:.1} s: shortest {:.0} ms, median {:.0}, longest \
         {:.0}; {} long (over {:.0} ms, the usual frame of the last {RECENT} being {:.0}); \
         each: {}",
        frames.len(),
        shortest / 1e3,
        median(frames.iter().copied()) / 1e3,
        longest / 1e3,
        long.len(),
        over / 1e3,
        usual / 1e3,
        each.join(" ")
    )];
    if !plain.is_empty() {
        let mean = plain.iter().sum::<f64>() / plain.len() as f64;
        let n = plain.len();
        out.push(format!("usual frame (the mean of {n} not long): {:.0} ms", mean / 1e3));
    }
    out
}

/// A frame ends: called at the end of the title's present call.
pub fn frame() {
    if !on() {
        return;
    }
    let now = Instant::now();
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = state.get_or_insert_with(|| State {
        last: None,
        since: now,
        frames: Vec::new(),
        recent: VecDeque::new(),
    });
    if let Some(then) = state.last.replace(now) {
        state.frames.push((now - then).as_micros() as f64);
    }
    let elapsed = state.since.elapsed();
    if elapsed < Duration::from_secs(5) || state.frames.is_empty() {
        return;
    }
    let frames = std::mem::take(&mut state.frames);
    state.since = now;
    for &us in &frames {
        if state.recent.len() == RECENT {
            state.recent.pop_front();
        }
        state.recent.push_back(us);
    }
    let usual = median(state.recent.iter().copied());
    for line in lines(&frames, usual, elapsed.as_secs_f64()) {
        tracing::info!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_measure_run_reads() {
        let frames = [150_000.0, 160_000.0, 140_000.0, 400_000.0, 150_000.0];
        let out = lines(&frames, 150_000.0, 1.0);
        assert_eq!(out.len(), 2, "{out:#?}");
        assert!(
            out[0].starts_with(
                "frame times: 5 frames in 1.0 s: shortest 140 ms, median 150, longest 400; 1 long"
            ),
            "{}",
            out[0]
        );
        assert!(out[0].ends_with("each: 150 160 140 400 150"), "{}", out[0]);
        assert_eq!(out[1], "usual frame (the mean of 4 not long): 150 ms");
    }

    #[test]
    fn long_is_half_as_long_again_and_at_least_40_ms_more() {
        assert_eq!(long_over(150_000.0), 225_000.0);
        assert_eq!(long_over(30_000.0), 70_000.0);
    }
}

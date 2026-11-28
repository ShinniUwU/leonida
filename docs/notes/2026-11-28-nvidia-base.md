# The VIctory-NVIDIA base: first unattended runs

The gpu, shader, hle and formats crates of trustno1-coder/VIctory-NVIDIA (updated to 63b902f,
renderer from codec-d/cool_fox f4b48ad) with our F1 menu, switch registry, shared settings file,
GTA V keys, cutscene skipper and frame generation on top (branch `feat/nvidia-base`, 9cfae40a).
Three unattended runs of the diagnostics build and one settings check of the release build.

The shader cache was warmed first (`leonida translate --warm all`): 25,985 shaders found, 25,760
translated, 225 do not translate, 133 s on 16 threads.

## Measured

RTX 3080 12 GB, driver 621.24, Ryzen 7 7800X3D, 2026-11-28. One run each with
`scripts/measure-run.ps1`, Series X, the shared settings file left out (as every measuring run).

| Run | fps rendered | fps shown | ms/frame | GPU ms | preparing ms | stable |
|-----|--------------|-----------|----------|--------|--------------|--------|
| 01-play | 12.5 | 12.5 | 80.2 | 26.5 | 31.6 | yes |
| 02-play-fsrfg | 11.9 | about 24 | 83.9 | 51.2 | 20.1 | yes |
| 03-cutscene | 12.3 | 12.3 | 81.0 | 49.0 | 17.0 | yes |

- 01-play: profiles base and nvidia-swap, `-SkipScenes -Seconds 450 -From 320`.
- 02-play-fsrfg: the same with dlss-framegen added (DLSS quality, frame generation).
- 03-cutscene: base and nvidia-swap, no `-SkipScenes`, `-Seconds 300 -From 170`.

Medians of the frame-rate lines from `-From` on (25 lines each). Draws per frame: 4,173, 3,900
and 4,187. Usual frame: 79, 82 and 71.5 ms.

**Frame generation (run 02).** DLSS upscaling (quality) did the upscaling in the measured window,
none failed and none was left to the title's FidelityFX. DLSS Frame Generation
does not run on an RTX 30 card (Streamline said so at start), so FSR 3 frame generation did: in
the measured window 1,583 presents with it on, none off, no prepare dispatch failed. FSR 3 shows
one generated frame between each two rendered, so the frame rate shown is about twice the
rendered one, about 24 fps. That figure is worked out, not counted: no log line counts the frames
FSR 3 shows.

**Stable.** All three ran until the script closed them (450, 450 and 300 s). No device removed or
hung, no panic, `crashes.log` did not grow. The world was reached at 66, 83 and 59 s.

**Slower than the swap test.** The swap test of the same card earlier the same day (crates at
VIctory-NVIDIA df2daaf, nvidia-swap with 45 switches) measured 16.6 fps (60.1 ms) in the opening
cutscene and 18.4 fps (54.4 ms) in play. These runs are 12.3 and 12.5 fps, 20 to 26 ms a frame
longer, more than the 5 ms one run varies by. Since then the crates went to 63b902f and the profile
gained five switches (`ALLOCATOR_TRIM`, `BUDGET_PACK`, `PASSES_HOT`, `SLOT_EVICT`,
`TEXSPK_PASS_MS`). Which of these costs the time was not measured; a run without the five
switches would tell the two apart.

### What the logs say about the picture

Counted in the measured window unless said, runs 01, 02 and 03 in that order.

- **Draws dropped:** about 90, 76 and 112 a frame without a host pipeline, and about 510, 480 and
  520 indirect ones. The log names ray tracing for almost all of them: hit shading without a BVH
  and the structure builder, both left out on purpose. Two a frame are the two shaders
  `LEONIDA_LEAVE_OUT` skips in nvidia-swap. Pipelines still being made: none at the median, up to
  6 (run 01) and 18 (run 03) a frame in the last lines.
- **Shaders that do not translate:** one compute shader in each run, at the start, its dispatches
  dropped.
- **Pipelines refused:** 29 "pipeline creation method that is not implemented" warnings in each
  run, all before the measured window.
- **Textures waiting:** textures picked by index waiting for upload, median 2,547, 1,179 and
  2,437, the most 3,202, 1,549 and 6,067. "The card has no room for more; they wait" once in each
  run, before the window. A texture picked by index changed after its upload and was uploaded
  again: 8, 16 and 4 times in the window (38, 63 and 24 in the whole run).
- **Black frames:** 21, 19 and 14 "black frame after a frame that was not" warnings, all in the
  first 81, 150 and 10 s (start screen, loading and the cutscenes held), none in the window.
- **Once in each run:** the host view heap reached its cap (views past it read a null slot), a
  buffer view and an unordered buffer view not on an element of their host copy, a constant
  buffer not aligned on the host, a texture copied back that could not be tiled, host memory
  pressure limiting texture prefetch.
- **Run 02 only:** Streamline logged once that `presentCommon()` was not observed. A host DLL of
  the title's FidelityFX upscaler's name was refused by the host policy; the title then loaded its
  own copy, bound with nothing missing.

The shots 1,500 frames after the world's first frame in runs 01 and 02 are black. Both fall on a
fade while cutscenes were held: run 01's has a black-frame warning at the same time, and in run 02
FSR 3 frame generation was switched off for those seconds. Later shots of the same runs and run
03's shots at 600, 1,000 and 1,500 frames have a picture. Nobody judged the pictures yet.

### Settings check

A settings file in the menu's format with only `LEONIDA_FRAME_INTERVAL_US=25000` (a 40 fps cap),
in a scratch `LOCALAPPDATA` so the player's own file was not touched:

- `leonida config --graphics` resolved it to 25000 with the source `settings`.
- The release build started with it for 100 s logged the frame interval as 25 ms against the
  title's 33.333 ms, and its frame-rate lines read 40.0 fps, 25.0 ms a frame at the start screen.
- `leonida config --switches-json` exits 0 in 28 ms with version 1, the settings file's path, 6
  groups and 30 switches (8 live), every one named `LEONIDA_` and with values. The launcher's
  parser on `feat/game-settings-sync` (6f0988a, `src/switch_list.rs`) reads the same fields with
  the same types, so it keeps all 30. Checked by reading its code; the launcher was not run.

# TODO

The goal: a console-like experience on PC. The game starts straight into play, holds a steady 30 fps (the console's
33 ms frame) or better, looks like it does on the console, loads shaders and textures in as smoothly as the retail
console does (no hitch when something is seen for the first time, no late or blurry textures), and doesn't crash.

## How to use this list

- **One item, one pull request.** Keep it small, and touch only the files the item needs.
- **Claim an item by opening a draft pull request** with its ID in the title (for example `P-3: ...`) as soon as
  you start. Search the open pull requests for the ID first. If someone already has it, pick another one.
- **The pull request that finishes an item deletes that item's row.** Don't edit other rows in it.
- **Progress without finishing:** a pull request that moves an item forward but doesn't finish it may edit that
  item's row only, to update its figures and link the new note. What was measured goes in the note under
  [docs/notes/](docs/notes/), not in the row. Never edit another item's row.
- **New item:** add a row at the end of its section with the next free ID. Never renumber.
- **Needs:** `game` means your own copy of the game, `GPU` a real Direct3D 12 card, `-` nothing beyond Rust.
- **Numbers** come from [docs/notes/](docs/notes/) and [docs/fork-notes.md](docs/fork-notes.md) unless a row says otherwise, and most were measured
  on one RTX 4070. Every figure you add to a row names the card, the resolution, the scene and the note it comes
  from; if the note doesn't name the card, say so. Prefer shares of a frame ("about half the card's time") to
  milliseconds, since they carry over to other cards better. Measure on your machine before and after a change,
  and put both figures in the pull request.
  Post figures, never log files: logs can quote game data (see [docs/asset-policy.md](docs/asset-policy.md)).
- **Focus right now: cutscenes.** They are the first thing a player sees. R-9 (they look right) and P-9 (they hold
  30 fps) come first, and fixes ported from other forks (UP-2 to UP-5) that help them are welcome.

## Rendering: looks like the console

| ID | Task | Where | Needs |
|----|------|-------|-------|
| R-1 | Everything out of direct sun is black: the ray-traced bounce light and lamp and sun shadows don't run. The structure-builder programs translate and 59 of 64 trace programs translate, but nothing is lit by them yet. | [docs/fork-ray-tracing.md](docs/fork-ray-tracing.md), `leonida-shader`, `leonida-gpu` | game, GPU |
| R-2 | The first gameplay room is sometimes not drawn after the cutscene hands over to gameplay. | `leonida-gpu` | game, GPU |
| R-3 | Washed-out frames turn up in about 6 of 21 runs with no switch on. Cause not known. | `leonida-gpu` | game, GPU |
| R-4 | An indirect compute pass asks for about 77,500 thread groups in x, where one dispatch runs 65,535. The rest is cut off; split the dispatch. | `leonida-gpu` | game, GPU |
| R-5 | Real queries, depth bounds, HDR output and scaling are missing. | `leonida-gpu` | game, GPU |
| R-6 | Render-target metadata planes are not handled. | `leonida-formats`, `leonida-gpu` | game, GPU |
| R-7 | Shaders that don't translate drop their draws. Count them again on this tree with `leonida translate --list` (the upstream counts in the fork notes are from 2026-11-23) and work down the list. Counted 2026-11-26 in the fork notes: the 195 `GS_VoxelizeGeneric` failures drop no draws (geometry programs are never translated); the 5 `CS_TraceShadowRays_*` need view addresses by heap slot from `leonida-gpu`. | `leonida-shader` | game |
| R-8 | Mapping pool pages in a row as one view made remaps cheaper but broke texture streaming (black or red faces, garbage in 3 of 3 runs). Find out why before trying it again; see P-4. | `leonida-hle` (`xmem`) | game, GPU |
| R-9 | Cutscenes should look like they do on the console. Run the opening cutscene unattended with `scripts/measure-run.ps1`, look at the shots, and add each glitch you see as a new row, one fix per pull request. R-2 (the room after the cutscene) and R-3 (washed-out frames) may turn out to be part of it. | `leonida-gpu`, `leonida-shader` | game, GPU |
| R-10 | On cards whose waves have 32 lanes (NVIDIA), console wave64 compute programs that read and write LDS inside one per-lane branch keep 64-lane waves and the card refuses them (a likely cause of a white veil over a cutscene). `LEONIDA_WAVE64_GROUP_WIDE=1` takes such branches in every thread where that is exact; try it in a cutscene run and count what still keeps 64 with `LEONIDA_WAVE_WHY=1`. | `leonida-shader` | game, GPU |

## Performance: a steady 30 fps

| ID | Task | Where | Needs |
|----|------|-------|-------|
| P-1 | Measure the frame-rate switches that `run-gta6.ps1` sets on more cards (AMD, NVIDIA 30 and 40 series, Intel). Turn on by default the ones that help everywhere without a device loss. Since #24 the run scripts take their switches from `gpu-profiles.json` by graphics card: add a profile for each card measured, with its figures. | `leonida-gpu`, `gpu-profiles.json` | game, GPU |
| P-2 | The per-record clamp before indirect draws is most of the card's time. About 2,400 indirect commands a frame still get a clamp pass, and about 1,000 mesh records sit scattered through a 100 MB argument buffer. | `leonida-gpu` | game, GPU |
| P-3 | Copies between the PC's memory and the card's are most of the submit thread's time. | `leonida-gpu` | game, GPU |
| P-4 | While the camera moves, about half the time goes to frames of about 490 ms in which the card is idle for about 330 ms. The title's page-mapping copies (`CopyPageMappingsBatchX` to `xmem::map_alias`) cost 32 ms of every cutscene frame and 246 ms of a streaming frame. | `leonida-hle` | game |
| P-5 | The title paces itself with `monitorx` / `mwaitx`, which only AMD processors have: about 280,000 emulated exceptions a second on an Intel processor. Find a cheaper way. | `leonida-loader` | game; an Intel CPU to measure |
| P-6 | Translated shaders are too expensive: the title's own passes as translated are about half of the frame (about 50 ms of 97 ms; first rooms, about 3,850 draws a frame, 2560x1440, with several off-by-default switches on; measured 2026-11-25, the card is not named in the note: [docs/fork-notes.md](docs/fork-notes.md), "what a frame is made of"). With every pass of the layer's gone, the first rooms would still be near 20 fps on that card. On an RTX 3080 12 GB (Ryzen 7 7800X3D, driver 621.24, first rooms), after #23 took the layer's own hash and compare passes from about 21 to 3.6 ms of card time a frame, the title's passes are about 95 ms of it. | `leonida-shader` | game, GPU |
| P-8 | About 24 GB of memory is in use during play. | all | game |
| P-9 | Cutscenes should hold 30 fps without stutter. Measure the opening cutscene with `scripts/measure-run.ps1` and write down where the frame goes. P-4's page-mapping copies already cost 32 ms of every cutscene frame, so start there. | `leonida-hle`, `leonida-gpu` | game, GPU |
| P-10 | `LEONIDA_WC_NO_BACK` forgets a write-combined region only when `XMemVirtualAlloc` gives the same addresses out again without the bit: the title's `VirtualFree` goes to the host unseen. Hook it and call `leonida_gpu::title_memory_write_combined(.., false)` for what it releases (foxhound's `title_memory_freed`). See `docs/notes/2026-11-27-wc-no-back.md`. | `leonida-hle`, `leonida-gpu` | - |

## Stability

| ID | Task | Where | Needs |
|----|------|-------|-------|
| S-1 | One device loss with indirect draw windows on (`LEONIDA_INDIRECT_WINDOW=1`), 44 s into the world. Cause not found. Don't combine it with `LEONIDA_CARD_FETCH_INFLIGHT=1`. | `leonida-gpu` | game, GPU |
| S-2 | A session past the first rooms lost the device in the open world with no driver entry ("device removed"). | `leonida-gpu` | game, GPU |
| S-3 | Menu frame pacing drifts on one screen in release builds. | `leonida-gpu` | game, GPU |

## Audio, input and system services

| ID | Task | Where | Needs |
|----|------|-------|-------|
| A-1 | Reverb is silent. | `leonida-hle` (audio) | game |
| A-2 | Opus decoding is not checked against what the title expects. | `leonida-formats`, `leonida-hle` | game |
| A-3 | Trigger rumble: an Xbox Series pad has small motors in the LT and RT triggers besides the two in the grips. XInput drives only the grip motors, so the title's trigger vibration is dropped (`shortcut:` in `gameinput.rs`). Windows.Gaming.Input (`Gamepad.Vibration`) can drive them. | `leonida-hle` (input) | game, an Xbox pad |
| A-4 | Speech is silent. The text entry dialog and the title relaunching itself are untested live. | `leonida-hle` | game |
| A-5 | Series S mode stops at the loading screen. | all | game |
| U-4 | Pad first: after launch, the game is playable without touching the keyboard. | `leonida-hle` (input) | game |

## Feel: smooth, and loads like retail

| ID | Task | Where | Needs |
|----|------|-------|-------|
| U-1 | One step from download to playing: build, warm the cache (P-7) and run, with no terminal knowledge needed. | `run-gta6.ps1` | - |
| U-2 | Fullscreen and borderless window, and a choice of output resolution. | `leonida-gpu` | GPU |
| U-8 | No hitch when a shader is seen for the first time. Shaders translate on first use, so a new area or effect can stall a frame even with a warm cache. Count the long frames in a run with a cold cache and a warm one, and find which shaders still translate during play. Not measured yet. | `leonida-shader`, `leonida-gpu` | game, GPU |
| U-9 | Textures load in like on the retail console: no black, blurry or late textures while the camera moves. Compare shots of the same scene a few seconds apart and write down what pops in. Not measured yet; R-8 and P-4 touch the same streaming path. | `leonida-hle` (`xmem`), `leonida-gpu` | game, GPU |

## Code health

| ID | Task | Where | Needs |
|----|------|-------|-------|
| C-3 | `cargo fmt --check` reports about 1,420 differences. Run `cargo fmt` once, in a pull request of its own, at a moment when no other pull request is open (it touches almost every file), then add the check to CI. | all | - |
| C-4 | 163 `shortcut:` markers where the code approximates the console (79 in `leonida-gpu`, 55 in `leonida-shader`, 23 in `leonida-hle`, 4 in `leonida-formats`, 2 in `leonida-loader`). Replace them with the console's real behaviour, one per pull request. | all | game |
| C-5 | Some style lints are switched off for this fork in the workspace `Cargo.toml`. Turn them back on and tidy the code they flag. | `Cargo.toml` | - |

## Upstream

Ports from these projects follow the rules in the Upstream section of [CLAUDE.md](CLAUDE.md): what is worth taking,
how to measure it, and where a card-specific switch goes.

| ID | Task | Where | Needs |
|----|------|-------|-------|
| UP-1 | Upstream [Marvisak/VIctory](https://github.com/Marvisak/VIctory), the original project (many forks on GitHub share the name VIctory; this is the one they all come from), has 265 commits this fork lacks (counted on 2026-11-25 with `git cherry`). Go through them and cherry-pick the ones worth taking: `git remote add victory https://github.com/Marvisak/VIctory.git`, `git fetch victory`, `git cherry-pick <hash>`, then `scripts/rename-from-upstream.ps1` and `cargo check --workspace --all-targets`. Candidates by title, none tried yet: `4ab20fc` (indirect budget, C-2), `534e1dd` (AVX2 check), `e9cb14a` (missing title DLLs message), `42961e8` (warns when GPU hang settings would freeze the PC), `a3acc87` (checks the required startup archive). Many of the GPU commits touch code victory-gta6 changed, so expect conflicts there. | whole tree | - |
| UP-2 | Belobug/VIctory-Belobug-AMD (branch `a-publier`, GPL-3.0, tuned for AMD only; about 22 fps on [PortWatch](https://portwatch-vi.xyz/)) was taken off GitHub on 2026-11-27. Its last known head is `4eb6cfd` (2026-11-26). The public mirrors checked that day stop earlier ([Dxhiio/VIctory-Belobug-AMD](https://github.com/Dxhiio/VIctory-Belobug-AMD) at `cd8dfc0`), so port from a clone that has `4eb6cfd`, or from a mirror and say which. Go through its merged pull requests and commits and port the useful ones, one (or a few related) per pull request, with the hash or pull request link in the description. Run `scripts/rename-from-upstream.ps1` after each (it uses `VICTORY_*` names). A switch that only helps AMD goes into an AMD profile in `gpu-profiles.json`, not `base`; check every port on an NVIDIA card too before calling it done. Its open pull requests went with the repository; the upstream ones they ported (VIctory #2012, #2825, #3053 and others) belong to UP-1. | whole tree | game, GPU |
| UP-3 | [stellarr-studio/kelvaroq](https://github.com/stellarr-studio/kelvaroq) (GPL-3.0, `research/` GPL-2.0-or-later) is an NVIDIA-only fork (about 10 fps on PortWatch, a 36-minute run without a crash on an RTX 4090; UP-5 now ranks higher). Its work is a numbered patch series against pinned upstream revisions, not plain commits, so port it patch by patch. Most patches are off by default there: measure each one before turning it on, and keep NVIDIA-only switches in an NVIDIA profile. Leave out its Python scripts (the policy forbids them) and its launcher. | whole tree | game, GPU |
| UP-4 | [DexNom/VIctory](https://github.com/DexNom/VIctory) (GPL-3.0, a fork of VIctory) is the best AMD fork still public on PortWatch: about 3 fps there, but the best picture score of all (0.85). avatarmanz/VIctory-7900XTX, listed here before, was taken off GitHub on 2026-11-27. Look for rendering fixes first, since they help R-9. Same rules as UP-2. | whole tree | game, GPU |
| UP-5 | [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA) (GPL-3.0) ranks first among public forks on PortWatch (about 20 fps, NVIDIA only, measured on one RTX 5070). Since 2026-11-26 it is built on [readmet3xt-lab/foxhound](https://github.com/readmet3xt-lab/foxhound) (GPL-3.0-or-later), so look in foxhound's history first for where a change began. Port what makes NVIDIA faster without changing the picture: the reworked descriptor slots (NVIDIA caps the view heap below what the title asks for; our debug layer shows the same 2,097,152 to 1,000,000), memory compares on the card, the faster base settings. Part of its speed comes from leaving console work out: the title's ray tracing is off with an estimate of indirect light and reflections in its place, two water light programs are dropped on 32-lane cards, and the title's 30 fps frame interval is lifted. Those go behind `LEONIDA_*` switches, off by default, in an opt-in profile for players who want them, never in `base`. On the local branch `feat/nvidia-base` its gpu, shader, hle and formats crates (63b902f) replace ours; there DLSS, FSR 3 and MFG Unlocker files are fetched only after a yes to a question that defaults to no (or by running the scripts by hand), from official releases, checked by hash or by the vendor's signature, and the title stays offline. Its F1 menu is replaced by ours (live switches only). Still left out: its D3D12 runtime and Agility SDK downloads and its launcher. Open: measure the swap against `base` before it replaces `main`. Same rules as UP-2 otherwise. | whole tree | game, GPU |
| UP-6 | [PastiiJay/klbf-lab](https://github.com/PastiiJay/klbf-lab) (GPL-3.0) is an AMD fork suggested by a community member; how good it is has not been checked. Port from its `stable-amd` branch (also `beta-amd`, `rdna2-test`): they branch off victory-gta6 `a26859f` (2026-11-25), as we do, so commits cherry-pick. `beta` and `main` share no history with ours and can only be ported by hand. `stable-amd` also carries the full history of avatarmanz/VIctory-7900XTX (UP-4; commits by author `local`, about 175), whose README reports 13 to 22 fps indoors on one RX 7900 XTX at 1440p, Series X profile; our #115 came from it (`b620284`). Candidates by title, none tried yet: the RDNA2 device-loss fix (klbf-lab #127: `867330f`, `e1414ec`, `680c779`, `f4c71c1`, `b6d692b`, `4c3d1bd`, `eb6feb8`), which goes in an RDNA2 profile; an AMD shader compiler crash (`366be7e`); the page-mapping copy freeze fix (`bc264a4`, P-4, P-9); fewer copies a frame (`6951355`, `76e8485`, `9557c0c`, P-3; the commit gives 86 to 36 MB a frame and does not name the card); memory fixes (`85076d8`, P-8); the high-performance adapter on laptops (`1eb1b47`). Leave out its launcher and its updates from GitHub, the Agility SDK download, the FSR DLL hooks, the bench tool, the branding and the Python scripts in `tools/ghidra-scripts/`. Same rules as UP-2 otherwise. | whole tree | game, GPU |

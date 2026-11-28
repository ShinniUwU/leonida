# UP-5: DLSS Frame Generation through NVIDIA Streamline (experimental, off by default)

On RTX 40 and 50 cards, DLSS Frame Generation can now show generated frames between the title's
own, on our present. It is a port of the frame generation half of
[trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA), whose DLSS
Super Resolution half came first ([note](2026-11-27-dlss-super-resolution.md)). Upstream commits,
all GPL-3.0-or-later:

- `fc8b9ca` (readmet3xt-lab): the Streamline proxies, the latency markers, the present path, the
  noted upscaling inputs, and the external step in the line of written-down list calls.
- `1d0fa87` (readmet3xt-lab): the colour without the interface on Series X, copied in front of
  the title's interface composite.
- `8a93538` (readmet3xt-lab): the per-second line of where a frame's time goes in Streamline.
- `22c6be2` (trustno1-coder): 2x to 6x, result 39 taken as set, both rates in the caption, the
  swap chain hook for DLSS alone (`LEONIDA_STREAMLINE_PRESENT`), the check's teardown.
- The DLSS Frame Generation parts of `29d8dbf`, `10862a6` and `df2daaf`: the start notice and
  the RTX 40/50 card test.
- AMD FSR 3 frame generation for the other cards (see [below](#amd-fsr-3-frame-generation)):
  `29d8dbf` (FidelityFX frame generation and its swap chain, chosen where DLSS Frame Generation
  does not run), `10862a6` (`scripts/get-amd-fsr.ps1` and the one-time questions at start) and
  `df2daaf` (FSR 3 frame generation needs DLSS upscaling).
- MFG Unlocker, someone else's mod, as an opt-in (see [below](#mfg-unlocker-someone-elses-mod)):
  `0cef07e` (the download script, the question, the menu row, the load) and the mod parts of
  `10862a6` (the start notice, DLLs beside the program).

Names were changed from `victory`/`VICTORY_` to `leonida`/`LEONIDA_`.

On the renderer swap (branch `feat/nvidia-base`, 2026-11-28) the gpu, shader, hle and formats
crates are VIctory-NVIDIA's at `63b902f` (from `df2daaf` on; renderer from codec-d/cool_fox
`f4b48ad`, history from readmet3xt-lab/foxhound), so DLSS Frame Generation and FSR 3 frame
generation are upstream's own code there, not this port. The sections on how it works describe
this port; upstream's code does the same in outline. The measurements below were taken on this
port (leonida-dlss `761059f1`) and do not carry over to the swap.

## The switches

All off by default; without `LEONIDA_FRAMEGEN=1` nothing of this runs and the present is the one
`main` has.

- `LEONIDA_FRAMEGEN=1`: frame generation. It needs `LEONIDA_DLSS=<mode>` too, and the line of
  submissions (`LEONIDA_ASYNC_SUBMIT=1`, which the base profile sets). Without DLSS it stays off
  with a line at the start: on Series X the title upscales only on the FidelityFX path DLSS puts
  it on, so there would be no depth and motion vectors to tag.
- `LEONIDA_FRAMEGEN_X=2..6`: frames shown for each one rendered (2 unless set; more than the
  card generates is cut to what it does, more than 2 needs an RTX 50). `0`: made ready, off.
- `LEONIDA_FRAMEGEN_HUDLESS=0`: no colour without the interface is copied or tagged (generated
  frames then warp the interface at the edges of moving things).
- `LEONIDA_FRAMEGEN_CYCLE=<presents>`: for a check with nobody at the keys, the amount is asked
  anew every that many presents, in turn 2, 4, off and 6.
- `LEONIDA_STREAMLINE_PRESENT=1`: DLSS without frame generation; our swap chain is Streamline's
  proxy of it so that Streamline sees the presents (no "presentCommon() was not observed").

The opt-in profile `dlss-framegen` sets `LEONIDA_DLSS=quality`, `LEONIDA_FRAMEGEN=1` and
`LEONIDA_TEXTURE_ROOM_PERCENT=80`. The 80 is upstream's launcher value with frame generation on
(it takes 1.5 to 2 GB of the card there), not a figure of ours.

`scripts/get-nvidia-dlss.ps1` now also fetches `sl.dlss_g.dll`, `nvngx_dlssg.dll`,
`sl.reflex.dll` and `sl.pcl.dll` from the same NVIDIA release, signature checked like the others;
at start every one of the eight that is in the folder must carry a valid NVIDIA signature.

`leonida dlss-check --present` asks for frame generation as well, makes the device (the queue
through the device proxy), runs DLSS on stand-in textures and presents 180 frames in a window
of ours with stand-in depth, motion vectors and colour without the interface tagged. It reports
the presents with frame generation on, the frames it showed and its status. It then shuts
Streamline down, which upstream found avoids a fast fail (0xC0000409) at exit.

## How it works

- **Device and queue**: after `slSetD3DDevice`, the direct queue is made through Streamline's
  device proxy (`slUpgradeInterface`), and the native queue behind it is the host's queue. The
  second queue, the indirect and tile queues stay native and sync with it by fences, as
  upstream's do. Reflex low latency is turned on (frame generation needs it); `slReflexSleep` is
  not called.
- **Swap chain**: made through Streamline's DXGI factory proxy on the queue proxy, so Streamline
  owns the presents. The system's Alt+Enter is not taken for the window, and frame generation is
  switched off before the swap chain's buffers are made again for another size or format.
- **Inputs**: each upscale dispatch DLSS takes also notes its depth and motion vectors on the
  host list. When that list is submitted they become the latest; the next present tags them in
  the states the submissions left them in (viewport 0), with constants made from the upscaling
  (the near and far planes put in order: the title names the far one first) and the latency
  markers. A present with no new upscaling (menus, loads) switches frame generation off and
  clears the tags.
- **The interface**: on Series X the title blends its interface over the frame with one
  triangle (shaders `0602c2384acbb3b2` and `a78d088fa55aa495`, from upstream's frame listings).
  In front of that draw, when its one target is a texture the title presented before, the
  target is copied into one of two textures of ours, and the present of that texture tags the
  copy as the colour without the interface. Other pipelines that draw into a presented texture
  are named in the log once each.
- **Line of written-down calls** (`LEONIDA_THIN=1`): DLSS's work is written into the list's
  stream and run when the stream is translated, in its place (`put_external`). Before this it ran
  at the call, out of order with the rest of the list. A stream let go of drops it unrun.
- **Log**: the start prints what this card does for frame generation. Each frame-rate line adds
  presents with it on and off, frames shown and status, and where the time goes in Streamline's
  calls; every tenth line, the copies of the colour without the interface. The window caption
  shows both rates while it runs.

## Differences from upstream

On the swap, upstream's code at `63b902f` runs, with these changes of ours:

- Frame generation needs DLSS (`apps/leonida/src/run.rs`): without it both kinds stay off with
  a line saying so. Upstream allows DLSS Frame Generation alone at the DLAA scale; its own
  FidelityFX code warns that the title then fails at other output sizes.
- `LEONIDA_DLSS` takes a mode's name (`off`, `1`, `dlaa`, `quality`, `balanced`, `performance`,
  `ultraperformance`), as the launcher and the profiles write it; `LEONIDA_DLSS_MODE` still wins.
- The default folders are the data folder's `nvidia\streamline-v2.14.16\bin\x64` and
  `amd\fidelityfx-v2.4.0`, where the download scripts put the files; upstream's defaults are an
  SDK folder under `C:\dev` and a `fidelityfx` folder beside the program.
- Every NVIDIA file of the folder is checked to be signed by NVIDIA, and AMD's by AMD, by the
  signer's name; upstream accepts any valid signature.
- The start questions default to no (`[y/N]`; upstream's to yes), and come from `leonida run`.
- The F1 menu is ours and shows only what applies at once: of frame generation, its amount
  (upstream's `set_framegen`). Its type (`LEONIDA_FSR_FRAMEGEN`), MFG Unlocker and the rest
  apply at a start and are in the Leonida launcher.
- The line of submissions is `LEONIDA_ASYNC_SUBMIT=1` (in `base`) or `LEONIDA_THIN=1`, as
  upstream's.

## AMD FSR 3 frame generation

On cards without DLSS Frame Generation (RTX 20 and 30, or with `LEONIDA_FRAMEGEN` unset), AMD
FSR 3 frame generation can show one generated frame between each two of the title's. It uses
AMD's FidelityFX API 2.x (`amd_fidelityfx_loader_dx12.dll` and
`amd_fidelityfx_framegeneration_dx12.dll` of FidelityFX SDK v2.4.0), loaded at run time from a
folder outside the repository. It is chosen as upstream chooses it: DLSS Frame Generation where
it runs, FSR 3 where it does not, never both.

### Switches

All off by default, read in `crates/leonida-gpu/src/streamline.rs` (`Asked::from_vars`) and
`crates/leonida-gpu/src/streamline/fsr.rs`:

- `LEONIDA_FSR_FRAMEGEN=1`: FSR 3 frame generation. It needs `LEONIDA_DLSS=<mode>` and the line
  of submissions (`LEONIDA_ASYNC_SUBMIT=1`); without either it stays off with a line at start.
  With `LEONIDA_FRAMEGEN=1` too, it is used only where DLSS Frame Generation is not ready.
- `LEONIDA_FIDELITYFX_DIR=<folder>`: where AMD's two DLLs are; default
  `<data_dir>\amd\fidelityfx-v2.4.0`, where `scripts/get-amd-fsr.ps1` puts them.
- `LEONIDA_FRAMEGEN_X`: `0` makes it ready but off; any other value gives 2x (FSR 3 does no
  more).
- `LEONIDA_FRAMEGEN_HUDLESS=0`: the colour without the interface is not copied or given.
- `LEONIDA_FSR_FRAMEGEN_DEBUG=1`: FidelityFX draws its tear and pacing lines over the picture.

The opt-in profile `dlss-framegen` now sets `LEONIDA_FSR_FRAMEGEN=1` next to
`LEONIDA_FRAMEGEN=1`, so one profile gives a player DLSS with frame generation: DLSS Frame
Generation on RTX 40 and 50, FSR 3 on other cards.

### Getting AMD's files

`scripts/get-amd-fsr.ps1` (from upstream's `10862a6`) shows what it fetches and asks first
(`-Yes` skips the question). It takes the two DLLs and AMD's licence from the `v2.4.0` tag of
AMD's FidelityFX SDK on GitHub (`Kits/FidelityFX/signedbin`). That release has no files of its
own: its notes give the tag's source archive (about 200 MB) as the download. The script takes
the same two files from that tag, so it does not fetch the whole archive. Each must match the
SHA-256 upstream pinned and carry a valid signature by AMD, or it is deleted. Files go to the
data folder, never into the checkout. At start the game checks AMD's signature again
(`WinVerifyTrust`) before it loads them.

`leonida run` asks once (from upstream's `offer_dlss` and `offer_fsr`), with a terminal only, when
DLSS is asked for and NVIDIA's files are missing from the default folder, and when FSR 3 would be
used and AMD's are missing. On yes, it runs the script carried in the executable from a fresh
folder under `%TEMP%`. On no, a `declined` file in `<data_dir>\nvidia` or `<data_dir>\amd` stops
the question.

### How it works

- **Swap chain**: when FSR 3 is ready, our swap chain is FidelityFX's frame interpolation swap
  chain (made on the host queue), in place of the plain one. Its context goes before the buffers
  are made again for another size.
- **Inputs**: the same ones DLSS Frame Generation gets. These are the depth and motion vectors
  noted at each DLSS upscale, the camera's planes and field of view, and the frame time, all
  from the last upscaling submitted before the present. The colour without the interface comes
  from the same copy in front of the title's interface composite.
- **Dispatch**: the frame generation context is made for the output size, the render size and
  the format, and made again when one changes. Each present records FidelityFX's prepare
  dispatch last in our present list. FidelityFX's swap chain calls back for the generated frame,
  and the callback dispatches it. A present with no new upscaling (menus, loads) has frame
  generation off.
- **Log**: every tenth frame-rate line counts the presents with it on and off, the dispatches
  that failed, and the colours without the interface given.

### Differences from upstream

On the swap, upstream's FSR 3 code at `63b902f` runs (it also refuses FSR 3 without DLSS), with
these changes of ours:

- The question at start defaults to no (`[y/N]`); upstream's defaults to yes. The script asks
  `[y/N]` too.
- The game checks that AMD signed the DLLs, by the signer's name; upstream accepts any valid
  signature. The script also checks it, which upstream's does not.
- The frame generation type is not in the F1 menu (it applies at a start): `LEONIDA_FSR_FRAMEGEN`
  is in the Leonida launcher.
- Not here: the play menu's warm-up, the Agility SDK and Windows 10 runtime downloads,
  `Get-AMD-FSR.cmd`, and upstream's needs-logs workflow.

## What was and was not measured

Written before the runs in [Measured](#measured), which ran DLSS, FSR 3 and MFG Unlocker in
the game.

Nothing ran with NVIDIA's files and nothing ran in the game: the only card here is an RTX 3080
12 GB (Ryzen 7 7800X3D, driver 621.24), which has no DLSS Frame Generation, and NVIDIA's files
are not on this PC. No frame times were taken.

Checked without them: unit tests for the switches (`LEONIDA_FRAMEGEN`, `_X` clamped to 2..6,
`_HUDLESS`, `LEONIDA_STREAMLINE_PRESENT`), frame generation staying off without DLSS, the RTX
40/50 card test, the start notice's reasons, the frame generation structure offsets, the near and
far planes put in order, that the DXGI factory proxy is asked only with frame generation and the
downloads (OTA) never, and that written-down DLSS work is run or dropped exactly once.

Limits and open points:

- No RTX 40 or 50 card ran this code. An RTX 3080 only ever gets the "needs an RTX 40 or 50
  card" line.
- The colour without the interface is a copy in front of one known draw of one build; other
  builds may name other shaders.
- HDR output (a 16-bit float swap chain) is not supported by frame generation (status 4).
- Frame generation holds a frame back; it is meant for a base frame rate of 60 and up.
- `slShutdown` is not called when the game ends; with frame generation on, the process may fail
  fast at its very end (0xC0000409), as upstream's check did without the shutdown.
- The proxied queue against the native second queue's fences was not tried on this tree.

## Measured

RTX 3080 12 GB, driver 621.24, Ryzen 7 7800X3D, Windows 11, series-x, 2026-11-28: the
diagnostics build of `761059f1`, the shader cache warmed first. NVIDIA's files came from
`scripts/get-nvidia-dlss.ps1` (Streamline v2.14.16) and AMD's from `scripts/get-amd-fsr.ps1`
(FidelityFX SDK v2.4.0), both answered yes, both passed their signature checks.
`leonida dlss-check` said: DLSS supported, DLSS Frame Generation not supported on this card
("no supported adapter found"), and the five Streamline DLLs it checks signed by NVIDIA.

Each run is one `scripts/measure-run.ps1` run, with the `nvidia-picture` profile under the
run's own profiles (through `LEONIDA_PROFILE`), closed by the script at its time limit. Play:
`-SkipScenes -Seconds 450 -From 320`, a shot 1,500 frames after the world's first. Opening
cutscene: `-Seconds 300 -From 170` (with `-From 320` a 300 s run counts no lines), shots at
600, 1,000 and 1,500 frames. One run each.

| Run | On top of `nvidia-picture` | Rendered fps | Shown fps | Frame generation | Stable |
|-----|----------------------------|--------------|-----------|------------------|--------|
| 01-base | nothing | 9.6 | 9.6 | none | yes |
| 02-dlss | `dlss` (quality) | 9.7 | 9.7 | none | yes |
| 03-dlss-fsrfg | `dlss-framegen` | 9.4 | about 19 | FSR 3, 2x | yes |
| 04-cutscene-base | nothing | 2.8 | 2.8 | none | yes |
| 05-cutscene-dlss-fsrfg | `dlss-framegen` | 5.8 | about 12 | FSR 3, 2x | yes |
| 06-mfg | `dlss-framegen`, `MFG_UNLOCKER=1`, `FSR_FRAMEGEN=0` | 9.5 | 19.0 | DLSS FG, 2x | yes |

Stable means no device lost, no crash report, no hang and no early exit. Every run reached the
world at 76 to 82 s.

- **Rendered fps** is the median of the frame-rate lines. **Shown fps** with FSR 3 is twice
  the rendered rate because our code counts two frames shown for each rendered one while it
  runs; FidelityFX's swap chain gives no count, so that figure is not measured. With DLSS
  Frame Generation it is measured: Streamline's frames presented, summed over the last 129 s,
  were 2,464 from 1,232 presents.
- **Frame generation ran** in 03, 05 and 06. In 03 FSR 3 was on for 99 to 100 of every 100
  presents at the end, all with the colour without the interface and no failed dispatch, and
  in 05 it was on through the whole cutscene. In 06 MFG Unlocker v1.7.2 (from
  `scripts/get-mfg-unlocker.ps1`, its SHA-256 matched) loaded, Streamline then reported DLSS
  Frame Generation supported, and it ran at 2x with status 0 on every line it was asked.
  It switched on 4 times and off 3 times around menus and loads, each time with result ok.
- **DLSS changes no fps here.** The title asks its upscaler for 2560x1440 to 3840x2160, and
  DLSS quality renders at the same 2560x1440. DLSS upscaled every frame and none failed. Of
  about 105 ms a frame, about 49 ms is preparing and 20 ms copying back on the CPU side,
  against 36 ms on the GPU.
- **GPU time per frame**: 36.0 ms base, 36.7 with DLSS, 39.6 with FSR 3, 41.2 with DLSS Frame
  Generation. The rendered rate stayed within the run-to-run spread.
- **The cutscene**: 2.8 against 5.8 fps is one run each, and the picture note found cutscene
  speed varies this much between runs. Not counted as a gain.
- **At about 10 rendered fps**, both kinds of frame generation show about 19, well under 30;
  both vendors mean them for a base of 60. Frame generation holds a rendered frame back, so it
  should add about one frame, around 105 ms here (a guess, not measured).
- **MFG Unlocker** wrote four files of its own (`RTX40MFG-*.ini`, `RTXMFG-*.log`,
  `RTXMFG-Universal.status.json`) beside `leonida.exe` in `target\diag`, as TODO C-6 says. They
  were moved out after the run. Whether the mod reaches the network was not checked.

### The picture

The shots were not judged by eye: the agent that ran these got them only as placeholders. The
figures, taken from the 960-wide copies:

- The four play shots at frame 1,500 (01, 02, 03, 06) have the same brightness (149 to 150 of
  255), share of black pixels (0.6 to 0.8%), of white (5 to 7%) and of flat 16 by 16 blocks
  (14 to 15%), and no blocks with a one-pixel checker. They differ from 01 by 17 to 19 levels on
  average; the moment is not quite the same.
- The cutscene shots are 11% black, which is the letterbox bars. Run 05's shot at frame 600 is
  dark, with 59% flat blocks. Both cutscene runs have such a dark, flat part near the start
  (36% and 58% flat at frame 2,048), so it looks like the scene, not the upscaling. Not
  confirmed.
- Every run's log shows the same picture problems, with or without DLSS or frame generation:
  about 80 draws a frame dropped for want of a host pipeline in play (125 to 144 in the
  cutscene), and about 4,000 textures waiting for the card (about 9,000 in the cutscene, 5,400
  in 06). They probably show as missing geometry and missing or blurry textures in every run (a
  guess from the log). So none of these runs can be called free of glitches.

So all six runs were stable. The most fps is DLSS with frame generation, about 19 shown from
about 9.5 rendered, and FSR 3 and DLSS Frame Generation through the mod are equal there. No
setting reaches 30 fps on this card, rendered or shown.

For FSR 3: nothing ran with AMD's files and nothing ran in the game. AMD's files were not
downloaded, and the script was not run. On the RTX 3080 here FSR 3 is the only frame generation
that can be measured; that is left to a run with the files. Checked without them: unit tests for
`LEONIDA_FSR_FRAMEGEN` and its need for DLSS, the start notice, the FidelityFX structure sizes
and offsets against the SDK headers, its states, formats and flags, missing or unsigned files
leaving it off, and the start question's answers and its choice of card. The signer name taken as
AMD's ("Advanced Micro Devices" or the word "AMD" in the certificate's name) is a guess; it was
not checked against AMD's real files.

## Settings menu and shared settings

One list of the switches a player may touch, `leonida_core::switches::SWITCHES`: each has its
name, a label, one line of help, a group, its values with labels, the value that does what the
code does without it, and whether it is live. `leonida config --switches-json` prints the list
(with the settings file's path) for the launcher, so neither side hard-codes it.

A switch is live only when changing it mid-game is safe: it is read every frame or at each use,
never decides device, heap, queue or swap-chain setup, and never feeds the shader cache key.
Live switches are read from atomics (`live_number`, `live_flag`), not from a value fixed at the
first read. The live ones:

| Switch | Menu | Default | Read in |
|--------|------|---------|---------|
| `LEONIDA_FPS_COUNTER` | FPS ON SCREEN | on | `leonida-gpu/src/fpsbox.rs`, every present |
| `LEONIDA_PANEL` | PERFORMANCE PANEL (also `F3`) | off | `leonida-gpu/src/panel.rs`, every present |
| `LEONIDA_FRAME_INTERVAL_US` | FRAME-RATE CAP | 0, the game's own | `leonida-gpu/src/frame.rs`, every frame wait |
| `LEONIDA_FRAMEGEN_X` | FRAME GENERATION AMOUNT | 2 | `leonida-gpu/src/streamline.rs`, every present |
| `LEONIDA_KBM_GTA` | GTA V KEYS AND MOUSE | off | `leonida-hle/src/modules/gameinput.rs`, every poll |
| `LEONIDA_MOUSE_SENS` | MOUSE SENSITIVITY | 1.0 | the same |
| `LEONIDA_MOUSE_INVERT_Y` | INVERT MOUSE Y | off | the same |
| `LEONIDA_MOUSE_CAPTURE` | MOUSE CAPTURE | off | the same |

Every other switch in the list needs a restart, and the launcher shows it as such. That
includes `LEONIDA_MENU` (on; 0 leaves `F1` and `F3` to the game). On the swap the menu shows
live switches only; its last row says the rest is in the launcher. `LEONIDA_FRAMEGEN_X` changes
the amount (OFF pauses it) only for frame generation that started with the game. Starting frame
generation, or choosing DLSS or FSR 3, needs a restart.

`LEONIDA_FRAME_INTERVAL_US` takes the place of the interval the title asks for (1,000 to
1,000,000 microseconds), after VIctory-NVIDIA 2f0fbe1. A change restarts the frame clock from
the next wait. Whether a shorter interval lets the title run past its own frame limiter was not
tried in the game (a guess).

### The shared settings file

`%LOCALAPPDATA%\leonida\settings\switches.json`, `{"version":1,"switches":{"LEONIDA_...":
"..."}}`. The launcher and the game both read and write it. A write goes to a temp file next to
it, which is then renamed over it (tried a few times if another program has the file open).
Values already there are kept, also names this build does not list, and only the changed ones
replaced. The read and the write of a save hold `switches.json.lock` (open for one handle
alone, deleted when it closes), which the launcher takes too, so neither writes over a change
the other made in between. When the game saves over a file it cannot read as one, it moves it
to `switches.json.bad` first (`.bad2` and on after that), as the launcher's Start fresh does;
a file it cannot open is not written over. A save that fails is tried again twice, 1.5 s
apart. At start the order is: a variable set by hand, then this file, then `leonida.toml`,
then the profiles (`crate::graphics`, source `settings` in the graphics line). Only names in the
list are taken from it. A damaged file, another version or another shape is left out with one
line on the console, never a crash; a missing file says nothing.
`LEONIDA_SAVED_SETTINGS=0` skips the file, and so does `LEONIDA_GRAPHICS_OFF=1`.
`scripts/measure-run.ps1` sets `LEONIDA_SAVED_SETTINGS=0`, so measuring runs compare like with
like.

The game writes the file 1.5 s after the last change in the menu, when the menu closes, and when
the title ends, whichever comes first.

### The F1 menu and the F3 panel

The menu is drawn in Leonida's look: a dark panel, paper text, flamingo for the frame and the
selected row, neon for the selected tab, teal for ON, amber for notes, and upper-case labels in
a 5 by 7 dot font of our own. There are four tabs: DISPLAY, FRAME GENERATION (with the type that
runs, shown but not changed), PICTURE (nothing live yet) and CONTROLS. Each row shows its value,
and the bottom line is the selected switch's help. The last row says that more settings, which
need a restart, are in the launcher. `Q` / `E` or a click changes tab. Up / Down or `W` / `S`
move. Left / Right or `A` / `D` change the value (stopping at the ends). `Enter` or a click
moves it on (round to the first). `Esc` or `F1` closes.

Keys and clicks come from the input poll in `leonida-hle` (every 8 ms). While the menu is open
the game gets no keys and no mouse. The mouse capture lets the cursor go, and the game's mouse
stands still. After the menu closes, keys stay away from the game until every key is let go, so the `Esc`
that closed it does not open the game's pause menu. `F1` and `F3` never reach the game while the
menu is enabled. `LEONIDA_MENU_OPEN=1` opens the menu at the start (for pictures of it).

The F3 panel is in every build, in the top left corner. It shows frames shown a second in large
type (with frame generation, the rendered rate is a line of its own), then the frame time, CPU
and GPU time per frame, draws, uploads, the card's memory (amber above 95% of the budget, with a
line when some sits in the PC's memory), dropped draws if any, frame generation and the cap. It
is updated once a second from the presenter's meter. The small fps counter is not drawn while
the panel shows.

Both are copied onto the back buffer after the title's frame, like the fps counter, so saved
frames and what is read back are unchanged. A picture becomes pixels only when it changes, in a
buffer that is never written again and is let go 120 presents after its last use.

Ported from VIctory-NVIDIA, reading its commits and files only:

- c899c02a (cherry-picked): the GTA V keyboard and mouse map, sensitivity, invert Y.
- 06afe63: mouse capture.
- 86a3976: keys and mouse kept from the game while the menu is open.
- 2f0fbe1: the frame interval, made live here.
- 6395727, 22c6be2 and 35c1560 (the panel part): the menu and the panel, read as a reference
  only. The drawing, the look, the tabs and the settings file are our own.

Limits and open points:

- shortcut: the panel colour is opaque (rgb 11, 8, 19, the design's rgba over black). The box
  is copied, and a copy cannot blend.
- With a 16-bit float (HDR) swap chain the colours are written as they are, not converted, as
  the fps counter does. They may look dimmer or brighter there.
- No key repeat: holding a key moves one step.
- A gamepad still reaches the game while the menu is open; only the keyboard and mouse are kept
  from it.
- Nothing here ran in the game. The menu, the panel, the file and the input gate were checked by
  unit tests on synthetic data only.

## MFG Unlocker (someone else's mod)

[MFG Unlocker](https://github.com/dashdogy/MFG-Unlocker) is a mod by dashdogy, under the MIT
licence, that lets DLSS Frame Generation run on GeForce RTX 20 and 30 cards. It is not ours,
not part of this repository, and not NVIDIA's: NVIDIA does not support frame generation on those
cards. It was not tested with this game, here or upstream (upstream ran only an RTX 50). Its
zip also carries Intel XeSS data under Intel's licence. Leonida Legacy can fetch and load it as
an opt-in, off by default, ported from trustno1-coder/VIctory-NVIDIA `0cef07e` and the mod
parts of `10862a6`.

### The switch

`LEONIDA_MFG_UNLOCKER=1` turns it on; unset or `0` (the default), nothing of it is fetched or
run. Read in `apps/leonida/src/mfg.rs` at the start of `leonida run`. It is a restart switch:
the Leonida launcher's MFG UNLOCKER puts it into the settings file (the F1 menu shows live
switches only). It does something only with DLSS and DLSS
Frame Generation asked for (`LEONIDA_DLSS=<mode>` and `LEONIDA_FRAMEGEN=1`, as the
`dlss-framegen` profile sets them); otherwise the start says it was not loaded.

### What happens at the start

1. **The question.** On an RTX 20 or 30 card, with DLSS and DLSS Frame Generation asked for and
   the switch never set (by hand, in the settings file, `leonida.toml` or a profile), `leonida
   run` asks once: `Turn MFG Unlocker on? [y/N]`. The question says that it is someone else's
   mod, that NVIDIA does not support it, and that it was not tested with this game. Only `y` or
   `yes` turns it on; an empty answer is no. A yes is saved in the settings file, as the
   launcher would save it. A no leaves a `declined` file in `%LOCALAPPDATA%\leonida\mfg-unlocker` and is
   not asked again. There is no question without a terminal (measuring runs), and none when a
   mod DLL already sits beside `leonida.exe`.
2. **The download.** With the switch on and the DLL missing, `leonida run` runs
   `scripts/get-mfg-unlocker.ps1` (carried in the executable, from a fresh folder under
   `%TEMP%`, with `-Yes`: turning it on was the yes). It fetches release v1.7.2 from the mod's
   GitHub releases into `%LOCALAPPDATA%\leonida\mfg-unlocker`. The zip must match
   `E002D624...7C893F22` and `MFGUnlocker.dll` in it `A9B1C017...766A11CF28F`, the SHA-256 its
   author published with that release (pinned upstream; not fetched again here), or nothing is
   kept. The licence and notice files in the zip are kept beside it; without one, `LICENSE` is
   taken from the repository at that tag (a guess at its name; if it fails the script says the
   licence is MIT and where to read it). Run by hand, the script asks `[y/N]` first.
3. **The load.** `leonida_hle::streamline::load_mod` opens the DLL without write sharing, checks
   its SHA-256 again (Windows' BCrypt) and loads it with `LoadLibraryExW` from its own folder and
   System32 only. This happens before Streamline is loaded or the device made, since the mod
   wraps NVIDIA's frame generation. A wrong hash or a failed load leaves the game running
   without it, with a line saying why.
4. **The notice.** Once it is loaded, the start line says so, that NVIDIA does not support it,
   and to turn it off in the Leonida launcher if the game fails. DLLs beside `leonida.exe` under the names of Windows' own it loads (`winmm.dll`,
   `version.dll`, `dxgi.dll`, `d3d12.dll`, `xinput1_4.dll`, `dinput8.dll`, `winhttp.dll`) are
   named at the start, since Windows would load them in place of its own.

To turn it off: MFG UNLOCKER, OFF in the Leonida launcher, then restart (or set
`LEONIDA_MFG_UNLOCKER=0`). The folder can be deleted.

### Differences from upstream

- Stricter at the load: upstream loads the DLL with `LoadLibraryW` and does not check it again.
  Here its SHA-256 is checked again at every load, and the search is its own folder and
  System32.
- Loaded only with DLSS and DLSS Frame Generation asked for, and asked only then. Upstream asks
  and loads with DLSS on, since its launcher had frame generation ready with DLSS.
- The question comes from `leonida run`, not a launcher of its own, and the choice goes into the
  shared settings file instead of `victory-settings.txt`.
- No `get-mfg` command: run `scripts/get-mfg-unlocker.ps1` by hand instead (it asks first).
  Deleting the `declined` file brings the question back.
- Not in the F1 menu (it applies at a start): MFG UNLOCKER is in the Leonida launcher, whose
  help says what upstream's `ABOUT_MFG` says.

### Not measured, and open points

Nothing of the mod was downloaded or run here, and nothing ran in the game. The checks are unit
tests: the cards asked (RTX 20 and 30 only), the answers (no by default), the pinned hash in the
Rust code matching the script's, DLLs beside the program found by name, the folder outside the
repository, the settings switch, SHA-256 against the standard test vector, and a load that
refuses a wrong hash. On the swap the load test only refuses a stand-in file of the wrong hash.

- The mod writes its own logs and settings (`RTXMFG-...`) beside the program. Built from a
  checkout, that is `target\release` inside it (ignored by git, but against the rule that
  derived data stays outside the repository). We cannot change where the mod writes (TODO
  C-6).
- Backspace opens the mod's own menu (upstream says so); whether the game takes that key too
  was not tried.
- Whether frame generation through the mod works with this game, and at what cost, is unknown.

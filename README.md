<p align="center">
  <strong><a href="https://t.me/projectleonidal">Telegram — Leonida Legacy</a></strong>
</p>

<h1 align="center">
  <br>
  <a>
  <img src="https://raw.githubusercontent.com/ShinniUwU/leonida/89b95f3e2215cc7b078618c2878ded1567a3be18/assets/logo.png" alt="Leonida Legacy" width="200">
</a>

  <br>
  <b>Leonida Legacy</b>
  <br>
</h1>

<h4 align="center">Grand Theft Auto VI (Xbox Series) running natively on Windows, from your own copy.
</h4>

<p align="center"><i>Continued by <a href="https://github.com/orkan1k">orkan1k</a>.</i></p>

> *ItsNotRexed just wanted to play on the PC.*

<p align="center">
  <a href="#status">Status</a> |
  <a href="#ready-made-builds">Ready-made builds</a> |
  <a href="#getting-started">Getting started</a> |
  <a href="#updating">Updating</a> |
  <a href="docs/notes/">Fork notes</a> |
  <a href="#contact">Contact</a> |
  <a href="#credits">Credits</a> |
  <a href="#license">License</a>
</p>

Leonida Legacy is a compatibility layer. The game's x86-64 code runs unchanged inside our process, and Leonida
Legacy supplies the console platform underneath it: the loader, the runtime services, a Direct3D 12 back end and a
shader recompiler that turns the console's GPU code into DXIL on first use. See
[docs/architecture.md](docs/architecture.md).

It is a fork of [victory-gta6](https://github.com/fxceless/victory-gta6), itself a fork of
[VIctory](https://github.com/Marvisak/VIctory).

> [!IMPORTANT]
> This repository **does not contain the game, keys or an executable**. You need your own copy of the game.
> Everything the tools make from your files (translated shaders, caches, saves, logs) stays outside the repository
> and on your PC. Never share those files.

**Made for everyone's PC, not just ours.** Many projects like this are tuned on the developer's own machine and
nowhere else. Leonida Legacy aims to run as well as it can on your hardware, and to be usable by people who can't
code and don't have an AI assistant to help them. The program picks its settings for your graphics card
(`gpu-profiles.json`), so a change that helps one card is not forced on everyone. If it runs badly on your PC, tell us
your CPU, GPU, driver version and frame rate (see [Contact](#contact)); that is how new cards get a profile.

## Status

**Not playable yet: frame rates are low, and shadowed areas are black.** Tested on Windows 11 Pro,
AMD Ryzen 7 7800X3D, NVIDIA RTX 3080 12 GB, 32 GB RAM, Series X profile. The world frame rates below were measured
by victory-gta6 on an RTX 4070 and have not been measured again on this machine yet.

| Stage | State |
|-------|-------|
| Boot, menus, start screen | works, 30 fps |
| Opening cutscene | plays from start to finish, lit |
| Saves | the game autosaves a checkpoint and loads it again |
| First gameplay scene | renders characters, textures, sky and the sunlit exterior; HUD, pause menu and pad input work |
| Light in shadow | missing: ray-traced bounce light and shadows don't run yet, so anything out of direct sun is black |
| Frame rate in the world | Earlier measurements: 2-4 fps without the newer switches, about 10 fps in the first rooms with them; the measured base profile now applies to direct launches too |

Known problems:

- The first gameplay room is sometimes not drawn after the cutscene hands over to gameplay.
- Around 24 GB of memory is in use during play.
- Many changes are specific to this game and this GPU; some are marked `shortcut:` in the code.

The full measurement log, what each switch does and the debugging options are in
[docs/notes/](docs/notes/) (newest) and [docs/fork-notes.md](docs/fork-notes.md) (up to 2026-11-27).

## Ready-made builds

If you just want to play and don't want to build anything:

1. Open [Releases](https://github.com/ProjectLeonidaL/leonida/releases) and download
   `leonida-windows-x64.zip` from the newest **Build** (the highest number).
2. Unpack it (right-click, **Extract All**) to a folder of its own, for example `C:\Leonida`.
3. Double-click `leonida.exe` in the `leonida` folder. If Windows says it protected your PC,
   click **More info**, then **Run anyway** (the file is not signed). On Windows 11 with Smart
   App Control on, Windows blocks it outright and shows no **Run anyway**.
4. The first time, it looks for your copy of the game in the usual places (`XboxGames`, the top
   two folder levels of each drive, Downloads and Desktop) and asks whether to use what it found.
   If it finds nothing, drag the game's folder from File Explorer into the window and press Enter.
   Then press Enter to warm up the shader cache, and the game starts.

You need Windows 10/11 x64, a CPU with AVX2 and a Direct3D 12 graphics card with shader model 6.6
and mesh shaders. When the game closes, or something goes wrong, the window says what happened and
waits for Enter.

**Updating a ready-made build:** download the newest Build zip and unpack it. From this build on,
a new build remembers your game folder (or finds it); copy `leonida.toml` from your old `leonida`
folder into the new one only if you changed settings in it. Saves stay in `%LOCALAPPDATA%\leonida`
either way. To find out whether there is a newer build, check
[Releases](https://github.com/ProjectLeonidaL/leonida/releases) for a higher Build number.

## Getting started

To build it yourself instead, you need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with
shader model 6.6 and mesh shaders, Rust (pinned in `rust-toolchain.toml`), and several GiB of free
Windows commit at startup (if `crashes.log` reports Windows error 1455, close other applications or
make the paging file bigger).

```powershell
cargo xtask install-hooks                # asset-policy check on every commit
cargo test --workspace                   # needs no game data

cargo build --release -p leonida
target\release\leonida.exe run --execute # asks for your game folder the first time
```

The first run asks for the folder of your copy of the game (the one with `MicrosoftGame.config` in it) and saves it to
`leonida.toml`, which git ignores. `leonida config` shows what is set, and `leonida.example.toml` lists the other
settings.

If startup names a missing game DLL, restore that file from your complete copy of the game, or choose the
folder containing the complete installation. Leonida checks required DLLs, including dependencies of other
DLLs, before running title code.

`run-gta6.ps1` does the build and the run in one step. The program itself selects the frame-rate settings from
`gpu-profiles.json`, including when launched directly through Wine. It asks for the game folder too, and whether
to remember it:

```powershell
powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1
```

To switch between branches and start `run-gta6.ps1` from a window instead, build the launcher once:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-launcher.ps1
```

It creates `Leonida-Launcher.exe` in the repository's root folder (git ignores it; it stays when you switch
branches). Start it, pick a branch and click **Load and start**: it switches to that branch and
runs `run-gta6.ps1`. Switching discards your changes to tracked files, after asking. If Git is
missing, the launcher offers to install it with winget; if the Visual Studio C++ build tools or
Rust are missing, the window that starts the game offers to install them first (the same check as
`start-measure.cmd`). Under **Settings** you can set your game folder (passed as `-GameDir`) and
the launcher's language (English or French); settings are kept in
`%APPDATA%\LeonidaLauncher\config.json`. The first build installs the `ps2exe` module for your
Windows user and downloads the logo; without network it uses a plain logo.

`scripts\measure-gta6.ps1` does the same with the diagnostics build, the frame log and the card meter on, and saves
everything the game prints to `%LOCALAPPDATA%\leonida\logs`. `-SpeedSwitches off` runs it without the frame-rate
switches or your graphics overrides, for a comparison in the same place.

### Experimental Linux launch with Steam Flatpak and Proton

Leonida still builds as a Windows executable; Proton runs it on Linux. On Pop!_OS 22.04 with
Proton 11.0, an RTX 5090 and NVIDIA 580.173.02, this path reached the opening scene with a
newer vkd3d-proton build, then stopped at a game resource-pool breakpoint. Gameplay on Linux
has not been validated. You need the Steam Flatpak with Proton installed, a working
PulseAudio-compatible desktop socket, and an x64 `d3dcompiler_47.dll` from a Windows SDK
installation. [The Windows SDK includes this DLL][sdk-d3dcompiler].

Build the Windows executable on Linux with [cargo-xwin][cargo-xwin], or use a Windows release
build you made yourself. Follow cargo-xwin's setup instructions for Clang/LLVM, then run in this
repository:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
cargo xwin build --release --locked --target x86_64-pc-windows-msvc -p leonida
```

Put the game executable named by `MicrosoftGame.config` beside that manifest and the rest of your
game files. On the tested NVIDIA PC, Proton 11.0's bundled vkd3d-proton 3.0.0 exited while
creating compute pipelines during world loading. For that combination, download vkd3d-proton
3.2.0 or newer from the [official package workflow][vkd3d-proton] and extract it outside this
repository. Its `x64` folder must contain matching `d3d12.dll` and `d3d12core.dll` files.
Then run:

```sh
./scripts/run-gta6-linux.sh \
  --game-dir "$HOME/Games/GTA6" \
  --proton "$HOME/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/Proton 11.0" \
  --compiler "$HOME/WindowsSDK/x64/d3dcompiler_47.dll" \
  --vkd3d-dir "$HOME/Downloads/vkd3d-proton/x64"
```

Replace those four paths with yours. `--exe` selects another Leonida Windows executable. The
launcher reads the repository's `leonida.toml` if present; `--config` selects another settings
file. It also reads `gpu-profiles.local.json` beside that file. It creates a dedicated Wine prefix
with Proton's DXVK/vkd3d graphics DLLs and your compiler DLL. The prefix, shader cache and saves
stay under `~/.local/share/leonida/proton` (or `$XDG_DATA_HOME/leonida/proton`). `--state-dir`
chooses another location outside the repository. For symlinks to files in another directory,
pass `--read-only-dir` for each target directory. `--bind-only` checks imports without starting
the title.

The launcher copies the newer vkd3d-proton DLL pair only into its dedicated Wine prefix. If
your Proton already includes a working version, omit `--vkd3d-dir` to use its bundled pair.

The script grants [Flatpak's PulseAudio socket][flatpak-sockets] and checks for its presence inside
Steam's sandbox before starting the game. If the check fails, restore your desktop's
PulseAudio-compatible audio service and Steam's audio permission. The title stays offline. This
launcher is for compatibility, **not malware isolation**: Steam's app data can still be visible
to Wine. Only run game files you trust.

[sdk-d3dcompiler]: https://learn.microsoft.com/en-us/windows/desktop/directx-sdk--august-2009-
[cargo-xwin]: https://github.com/rust-cross/cargo-xwin
[flatpak-sockets]: https://docs.flatpak.org/en/latest/flatpak-command-reference.html
[vkd3d-proton]: https://github.com/HansKristian-Work/vkd3d-proton/actions/workflows/artifacts.yml

### Graphics settings

The measured base profile is automatic. Experimental profiles such as `card-windows` remain opt-in.
To change a setting, add a `[graphics]` section to your `leonida.toml`; the example file lists every option:

```toml
[graphics]
async_submit = false
queue2 = 0
```

Only the options you write change. To disable automatic profiles, use `profiles = ["none"]` (or `profiles = []`)
in that section. Other settings you explicitly write still apply. To choose profiles yourself, use a list such
as `profiles = ["base", "my-test"]`; named profiles include the base settings and apply in catalog order.

For the optional NVIDIA shader compatibility fix, select `nvidia-code-tables`:

```toml
[graphics]
profiles = ["nvidia-code-tables"]
```

If you already have a `[graphics]` section, add or update its `profiles` line instead of adding
another section. Keep any profile names you already selected, then launch normally. Remove
`nvidia-code-tables` to disable it. This option is off by default. On the tested RTX 5090 with
Proton 11.0, it restored creation of six refused compute
pipelines; it does not establish correct gameplay or a frame-rate improvement. See the
[validation details](docs/fork-notes.md#r-7-large-code-literal-tables-on-nvidia-2026-11-26).

#### VIctory-NVIDIA's settings (`nvidia-swap`, NVIDIA cards, off by default)

The `nvidia-swap` profile turns on the speed switches trustno1-coder/VIctory-NVIDIA uses. On one
RTX 3080 12 GB it gave 12.5 fps in play and 12.3 fps in the opening cutscene, against 9.2 and 3.4
fps for `base` alone ([the note](docs/notes/2026-11-28-nvidia-base.md)). No other card was
measured. It leaves out two shaders (decal heap tidying and heat haze), so the picture is not
quite the console's.

1. Open `leonida.toml` in Notepad (beside `leonida.exe` in a ready-made build, in the checkout
   when you build it yourself).
2. Add these two lines at the end, or, if it already has a `[graphics]` section, add
   `"nvidia-swap"` to its `profiles` line:

   ```toml
   [graphics]
   profiles = ["nvidia-swap"]
   ```

3. Save and start the game as usual. To turn it off, remove `"nvidia-swap"` again.

#### DLSS Super Resolution (NVIDIA RTX, experimental, off by default)

DLSS can do the game's upscaling instead of its own FidelityFX upscaler. It ran on one RTX 3080
only together with frame generation (below); on its own nothing was measured, so treat it as
something to try. NVIDIA's files are not part of this
repository; you download them once yourself:

1. Run `scripts\get-nvidia-dlss.ps1` in PowerShell. It shows what it will get (NVIDIA Streamline
   from github.com/NVIDIA-RTX/Streamline, about 70 MB) and where it puts it (your Leonida data
   folder, outside this checkout), asks before downloading, and deletes any file NVIDIA did not
   sign. `leonida run` with DLSS on also offers this once (Enter alone means no); the game
   itself never goes online.
2. Run `leonida dlss-check --device` to see whether the files load and your card offers DLSS.
3. Turn it on with the `dlss` profile (quality mode):

   ```toml
   [graphics]
   profiles = ["dlss"]
   ```

   or with `LEONIDA_DLSS=quality` (also `balanced`, `performance`, `dlaa` or `off`).

To turn it off, remove `"dlss"` from the `profiles` line (or unset `LEONIDA_DLSS`). Without the
switch nothing changes and no NVIDIA file is opened. If something is missing (the files, a valid
NVIDIA signature, DLSS on your card), the game keeps its own upscaling and says why at start. If
the picture stops updating with DLSS on, close the game and turn it off. See
[the note](docs/notes/2026-11-27-dlss-super-resolution.md) for details and limits.

#### DLSS Frame Generation (NVIDIA RTX 40 and 50, experimental, off by default)

On an RTX 40 or 50 card DLSS can also show generated frames between the game's own (2x; up to 6x
on an RTX 50). It needs DLSS upscaling on, and it has not been run in the game by anyone yet.
Older cards (RTX 20 and 30) don't have it, and the game then says so at start and runs as before.

1. Run `scripts\get-nvidia-dlss.ps1` again if you got the DLSS files before: it now also fetches
   frame generation's four files.
2. Run `leonida dlss-check --present` to see whether your card offers it. It opens a window for
   three seconds and says how many frames frame generation showed.
3. Turn it on with the `dlss-framegen` profile (DLSS quality mode and frame generation 2x):

   ```toml
   [graphics]
   profiles = ["dlss-framegen"]
   ```

Turn on Windows' hardware-accelerated GPU scheduling first (Settings > System > Display >
Graphics > Change default graphics settings); frame generation needs it, and the `base`
profile's `LEONIDA_ASYNC_SUBMIT=1` (or `LEONIDA_THIN=1`). More frames per rendered one:
`LEONIDA_FRAMEGEN_X=3` (2 to 6), or `F1`, Frame generation, while the game runs. To turn it off, remove `"dlss-framegen"` from the
`profiles` line. Frame generation holds one frame back, so it feels best when the game already
runs at 60 fps or more. See [the note](docs/notes/2026-11-27-frame-generation.md).

#### AMD FSR 3 frame generation (other cards with DLSS, experimental, off by default)

On an RTX 20 or 30 card the `dlss-framegen` profile uses AMD FSR 3 frame generation instead (2x
at most). It also needs DLSS upscaling on. On one RTX 3080 with `nvidia-swap` it showed about 24
fps from 11.9 rendered, and ran without a crash for 450 s. The two profiles go together as
`profiles = ["nvidia-swap", "dlss-framegen"]`.

1. Run `scripts\get-amd-fsr.ps1` in PowerShell. It shows what it will get (two DLLs of AMD's
   FidelityFX SDK v2.4.0, from AMD's GitHub, and AMD's licence) and asks first. It checks each
   file's SHA-256 and AMD's signature and puts them in the data folder, outside this checkout.
   `leonida run` with the profile on also offers this once (Enter alone means no).
2. Turn it on with the same `dlss-framegen` profile.

The start shows a `frame gen` line saying which frame generation is used. To use FSR 3 without
the profile, set `LEONIDA_DLSS=quality` and `LEONIDA_FSR_FRAMEGEN=1`. To turn it off, remove the
profile, or set `LEONIDA_FSR_FRAMEGEN=0`.

#### MFG Unlocker (someone else's mod, RTX 20 and 30, off by default)

[MFG Unlocker](https://github.com/dashdogy/MFG-Unlocker) is someone else's mod (MIT licence) that
lets DLSS Frame Generation run on RTX 20 and 30 cards. NVIDIA does not support that, it is not
part of Leonida Legacy, and nobody has tried it with this game. You can try it if you want:

1. Use the `dlss-framegen` profile as above.
2. Answer `y` when the start asks `Turn MFG Unlocker on? [y/N]` (asked once, on RTX 20 and 30
   cards only; Enter alone means no), or turn MFG UNLOCKER on in the Leonida launcher.
3. The next start downloads it from the mod's own GitHub release (v1.7.2, about 5 MB, checked
   against the SHA-256 its author published) into `%LOCALAPPDATA%\leonida\mfg-unlocker`, with
   its licence, and loads it. `scripts\get-mfg-unlocker.ps1` does the download by hand.

If the game crashes or frame generation fails, turn it off: MFG UNLOCKER, OFF in the Leonida
launcher (or `LEONIDA_MFG_UNLOCKER=0`), and start again. The mod writes its own `RTXMFG-...` files beside
`leonida.exe`. Details: [the note](docs/notes/2026-11-27-frame-generation.md#mfg-unlocker-someone-elses-mod).

Environment overrides take precedence over TOML settings, which take precedence over profiles. For example,
an explicit `LEONIDA_QUEUE2=0` still wins. `LEONIDA_PROFILE` overrides the profile list, except the existing
diagnostics value `1`, which enables the CPU sampler and leaves graphics selection to the config. Run
`leonida config --graphics` to see the selected adapter, profiles, effective values and their sources as JSON;
it needs no game files. The scripts use this same result and leave your environment overrides intact.

The standard catalog is built into the executable. Put local profiles in `gpu-profiles.local.json` beside
your selected config file (the current folder when no config exists). They apply after the built-in catalog.
Profiles match the adapter used by Direct3D, including Wine's reported adapter identity. If discovery fails,
only profiles without card-specific conditions match. A local profile can cancel an earlier profile value
with `null`, but cannot override an explicit TOML or environment value. Game/cache/data paths, logging, RenderDoc startup
and WARP selection remain separate from graphics profiles.
GPU name patterns support case-insensitive `*`, `?`, `[abc]` and `[a-z]`; malformed patterns and unsupported
backtick escapes or negated character classes report an error. Unknown profile names also report an error.

### Testing for the project, with no programming experience

1. Download this repository (Code, Download ZIP) and unpack it, or clone it.
2. Double-click `start-measure.cmd`. If the Visual Studio C++ build tools or Rust are missing, it offers to install
   them with winget (about 3 GB, 5 to 15 minutes, asks first). The first build takes about 5 to 10 minutes.
3. Say yes when it offers to look for your copy of the game, and check the folder it found. If it finds nothing, drag
   the game's `.exe` from File Explorer into the window and press Enter. Answer its other questions, and play until
   the place you want to test.
4. Close the game window. The script offers a short test report: your graphics card, driver, CPU, RAM, Windows
   version, the switches that were on and the frame rates. It shows all of it before anything is shared, and never
   includes log files, game files, paths or your user name.
5. If you agree, it copies the report and opens `#test-results` on our [Discord](https://discord.gg/dxMD2XqyEJ).
   Paste it with Ctrl+V and send it. Nothing is uploaded unless you post it yourself.

Run it once with the speed switches on and once with them off, in the same place: the pair is what lets us add a
profile for your card to `gpu-profiles.json`. Reports are kept in `%LOCALAPPDATA%\leonida\reports`.

`scripts\shot-run.ps1` runs the game unattended and takes frame shots (`-ShotAt`, with `-Keys` to get through the menus) into
`%LOCALAPPDATA%\leonida\shots`. Shots come from your game: keep them on your PC.

If the GPU driver crashes while building one shader's pipeline, `LEONIDA_SKIP_PIPELINES=<16-digit shader hash>` keeps
that pipeline from ever reaching the driver (its draws are dropped).

To skip the opening's cutscenes (off by default, in every build), put this in a file named
`gpu-profiles.local.json` beside your `leonida.toml`, normally in the checkout, and start the game as usual. If the
file already exists, add only the object to its `profiles` list; delete that object to turn skipping off. If the
`[graphics]` section of your `leonida.toml` has a `profiles` line, add `"skip-scenes"` to that list too, or it is
left out. The game's console window lists `skip-scenes` after `profiles:` when it is on. While a letterboxed
cutscene shows, Enter is held for you ([details](docs/notes/2026-11-27-skip-scenes.md)). A measurement run with
the speed switches off leaves every profile out, this one too.

```json
{
  "profiles": [
    { "name": "skip-scenes", "env": { "LEONIDA_SKIP_SCENES": "1" } }
  ]
}
```

Saves, settings and `crashes.log` are kept in `%LOCALAPPDATA%\leonida\data`, derived data in
`%LOCALAPPDATA%\leonida\cache`.

Input: XInput pads. With none connected, the keyboard acts as a pad while the game's window has focus:

| Keys | Pad |
|------|-----|
| W A S D / I J K L | left / right stick |
| arrows | D-pad |
| Enter, Backspace, X, Y | A, B, X, Y |
| Q / E, Z / C | shoulders, triggers |
| F / H | stick clicks |
| Esc, Tab | Menu, View |

To play with keys and mouse laid out like GTA V on PC (off by default):

1. Start the game and press `F1`.
2. Press `E` until Controls shows, and turn on GTA V KEYS AND MOUSE and MOUSE CAPTURE.
3. Press `F1` to close the menu. Both stay on for the next start.

The keys are then `Shift` = A, `Space` = X, `R` = B, `F` = Y, `Tab` = LB, `Q` = RB, `Ctrl` = L3,
`C` = R3, `E` = D-pad right, `M` = D-pad up, `Z` = D-pad down, `G` = D-pad left, `V` = View, `P`
or `Esc` = Menu (letters by their place on the keyboard, as in the table above). The right and
left mouse buttons are LT and RT, and the mouse turns the camera. From
Belobug/VIctory-Belobug-AMD (pull request #295 by zhu1025960209-web).

`F` gets in and out of a vehicle and also switches the keys to driving: then `W` drives, `S`
brakes and reverses, `Space` is the handbrake and the mouse buttons aim and shoot from the
vehicle. The keys can't tell when the game puts you in a vehicle or takes you out by itself, or
when `F` found no vehicle near: if `W` and `S` do the wrong thing, press `F2` once. The frame rate
in the corner (FPS ON SCREEN in `F1`) shows ON FOOT or DRIVING.

MOUSE CAPTURE keeps the cursor hidden inside the game's window while it is in front, so a click
never lands on another window or screen. Hold `Alt` to get the cursor back (for the close button,
the taskbar or another window); switching to another window or opening `F1` lets it go too.
MOUSE SENSITIVITY and INVERT MOUSE Y are in the same tab. From trustno1-coder/VIctory-NVIDIA
(06afe63, e68f48a). The same switches can be set as `LEONIDA_KBM_GTA=1`,
`LEONIDA_MOUSE_CAPTURE=1`, `LEONIDA_MOUSE_SENS` and `LEONIDA_MOUSE_INVERT_Y=1`.

`F1` opens the settings menu while the game runs. It shows only what applies at once: Display (fps
on screen, the performance panel, a frame-rate cap), Frame generation (its amount), Picture and
Controls (the GTA V keys, mouse speed, invert Y, mouse capture).
`Q` / `E` change tab, the arrows or `W` / `S` move, the arrows or `A` / `D` change a value, `Enter` or a click moves
it on, `Esc` or `F1` closes. While it is open the game gets no keyboard or mouse. A change applies at once and is
saved to `%LOCALAPPDATA%\leonida\settings\switches.json`, which the next start reads (a variable you set by hand
still wins). Settings that need a restart are in the Leonida launcher. `F3` shows a performance panel
(`LEONIDA_PANEL=1` from the start). `LEONIDA_MENU=0` leaves `F1` and `F3` to the game;
`LEONIDA_SAVED_SETTINGS=0` ignores the saved file. Details: `docs/notes/2026-11-27-frame-generation.md`.

### Testing the optional one-record clamp

An experimental optimization reduces GPU work before individual indirect draws. It improved a synthetic
RTX 5090 benchmark by 19.4%; gameplay and other cards are unmeasured, so it stays off by default.
[The measurements and limits are here](docs/fork-notes.md#newest-2026-11-26).

To compare it using your configured game, create `gpu-profiles.local.json` beside your selected config file
(normally in the checkout) with the following content. If
the file already exists, add only this profile object to its existing `profiles` array:

```json
{
  "profiles": [
    {
      "name": "single-clamp-test",
      "opt_in": true,
      "match": {},
      "env": { "LEONIDA_INDIRECT_SINGLE": "1" }
    }
  ]
}
```

Run this pair in PowerShell from the checkout. Both use the base settings; the second adds the new clamp:

```powershell
cargo build --profile diag -p leonida --features diagnostics
$clampSeries = Join-Path $env:LOCALAPPDATA 'leonida\runs\single-clamp'
powershell -ExecutionPolicy Bypass -File scripts\measure-run.ps1 `
  -Root $clampSeries -Label 01-base -Profile base
powershell -ExecutionPolicy Bypass -File scripts\measure-run.ps1 `
  -Root $clampSeries -Label 02-single -Profile single-clamp-test
```

Repeat the pair with new labels before judging frame times. Compare the pictures locally and share only
the figures from `summary.txt`. Ordinary launches keep this profile off.

## Updating

Get the newest version before you report a problem: it may already be fixed. With a ready-made
build, see [Ready-made builds](#ready-made-builds). In PowerShell, in the folder you cloned:

```powershell
git switch main
git pull
powershell -ExecutionPolicy Bypass -File .\run-gta6.ps1
```

`run-gta6.ps1` builds the new version before it starts the game. Your game folder, saves and settings are outside
the code (`leonida.toml` and `%LOCALAPPDATA%\leonida`), so updating keeps them.

- **First time only:** if you cloned before the project moved to ProjectLeonidaL, point your copy at it once:
  `git remote set-url origin https://github.com/ProjectLeonidaL/leonida.git`. If `git pull` then says your branch
  has no upstream, run `git branch -u origin/main` once.
- **`git pull` refuses because files changed:** you edited something in the folder. `git stash` puts your changes
  aside, then `git pull` again; `git stash pop` brings them back. Your own GPU settings belong in
  `gpu-profiles.local.json`, which git leaves alone.
- **The first run after an update stutters** while shaders are translated again. Let the warm-up run when the script
  asks, or play a minute and start again.
- **Don't have git?** Download the code as a ZIP from the green *Code* button on GitHub, unpack it over a fresh
  folder, and copy your `leonida.toml` across.

## Contact

- **Discord:** [discord.gg/dxMD2XqyEJ](https://discord.gg/dxMD2XqyEJ)
- **Telegram:** [@wouldscams](https://t.me/wouldscams)
- **News & updates:** [Leonida Legacy on Telegram](https://t.me/projectleonidal)

## Credits

- **[victory-gta6](https://github.com/fxceless/victory-gta6)** by fxceless is the fork this repository continues
  under the name Leonida Legacy, with its history kept.
- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its contributors is the base of everything
  here: the loader, the runtime services, the Direct3D 12 back end and the shader recompiler.
- **[XCL](https://github.com/ivall/xcl)** by ivall and its contributors (GPL-2.0-only) was used as a reference for
  how the same game behaves under another layer. Fixes inspired by it were written anew in VIctory's code.
- The guard over every view of a memory page is code by **myokex**
  ([victory-amd](https://github.com/myokex/victory-amd), commit `c6dc4d8`), which adapts pull request #115 by
  Spexivus, its correction by joroyt and a part of VIctory #1122 by tightfall. The read-beside fix for kernel reads
  into title memory is by **Joshua Odom** (commit `10ea38e` of the same fork family).
- DLSS Super Resolution is ported from
  **[trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)** (commit `959dcaf`):
  the Streamline bindings and the DLSS path are work of **readmet3xt-lab** (commit `fc8b9ca`), and the
  FidelityFX upscaler path, its renderer sites and the host work in the title's command list come from
  **DexNom**'s fork ([DexNom/VIctory](https://github.com/DexNom/VIctory)). DLSS Frame Generation is ported
  from the same fork: the proxies and present path (`fc8b9ca`), the colour without the interface (`1d0fa87`)
  and the frame timing line (`8a93538`) by **readmet3xt-lab**, 2x to 6x and the check's teardown (`22c6be2`)
  by **trustno1-coder**. NVIDIA DLSS and Streamline are NVIDIA's; their files are downloaded by the user and
  are not part of this repository.
- On the local branch `feat/nvidia-base` the gpu, shader, hle and formats crates are those of
  **[trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)** (`df2daaf` to
  `63b902f`), whose renderer comes from **codec-d**'s cool_fox (`f4b48ad`) and whose history runs through
  **[readmet3xt-lab/foxhound](https://github.com/readmet3xt-lab/foxhound)**; the memory stack there is by
  **skornuta** (victory-memory-stack `57fb136`) and the PS4 pads by **FrancescoGelosoEterea** (VIscosity
  `d8db7a0`). [MFG Unlocker](https://github.com/dashdogy/MFG-Unlocker) is by **dashdogy** (MIT); it is not
  part of this repository.
- Logo: ["Grand Theft Auto VI" icon by SlyDevil](https://www.steamgriddb.com/icon/122618) on SteamGridDB.
- The victory-gta6 changes were developed with [Claude Code](https://claude.com/claude-code).

Grand Theft Auto and all related names are trademarks of their owners. This project is not affiliated with or
endorsed by them.

## License

GPL-3.0-or-later, like victory-gta6 and VIctory. See [LICENSE](LICENSE).

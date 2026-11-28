# DLSS Super Resolution through NVIDIA Streamline (experimental, off by default)

On NVIDIA RTX cards, DLSS Super Resolution can now do the title's upscaling in place of its
FidelityFX upscaler. It is a port of the DLSS Super Resolution part of
[trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA) at `959dcaf`.
There, readmet3xt-lab wrote the Streamline integration (`fc8b9ca`), and the FidelityFX upscaler
path, its renderer sites and the host work recorded into the title's command list come from
[DexNom/VIctory](https://github.com/DexNom/VIctory). Everything is GPL-3.0-or-later. Names were
changed from `victory`/`VICTORY_` to `leonida`/`LEONIDA_`.

On the renderer swap (branch `feat/nvidia-base`, 2026-11-28) the Streamline code is upstream's
own at `63b902f` (renamed), not this port; `leonida run` starts it as described here, with the
scene's scale patched by `apps/leonida/src/upscaler_path.rs`. What this branch adds to upstream's
code is listed in the frame generation note's
[differences](2026-11-27-frame-generation.md#differences-from-upstream). Nothing below was
measured on the swap.

## The switch

`LEONIDA_DLSS=<mode>`, one of `off`, `quality`, `balanced`, `performance` or `dlaa`, sets the
render scale: 2/3, 0.58, 0.5 or 1 of the output. The opt-in profile `dlss` in
`gpu-profiles.json` sets `quality` and matches no card by itself. Turn it on with
`[graphics] profiles = ["dlss"]` or `LEONIDA_PROFILE=dlss`. If the switch is unset or `off`,
nothing changes: no image patch, no FidelityFX interposer and no NVIDIA file is opened.

These switches are only for tuning, and all have defaults:

- `LEONIDA_DLSS_PRESET`: `default`, `e`, `f`, `j`, `k`, `l` or `m`, the DLSS model.
- `LEONIDA_STREAMLINE_DIR`: a folder of the NVIDIA DLLs other than the default
  `<data dir>\nvidia\streamline-v2.14.16\bin\x64`.
- `LEONIDA_DLSS_JITTER` and `LEONIDA_DLSS_MV`: `x,y` multipliers for the jitter and the motion
  vectors. Use them if a run shows a wrong sign or scale.
- `LEONIDA_DLSS_REACTIVE=0`: keeps the reactive mask away from DLSS.
- `LEONIDA_STREAMLINE_LOG=<folder>`: Streamline's own verbose log. The folder must be outside
  the checkout.

`leonida dlss-check [--output WxH] [--device]` reports whether the files load and are signed and
whether this card supports DLSS, in the `LEONIDA_DLSS` mode (`quality` if unset). With `--device`
it also makes the host device and runs DLSS three times on stand-in textures. It needs no game
files.

## How it works

- **NVIDIA's files** are never in the repository and the game itself never goes online. The
  user runs `scripts/get-nvidia-dlss.ps1` once (it asks first), or answers yes when `leonida run`
  offers to run it. It fetches Streamline 2.14.16 from github.com/NVIDIA-RTX/Streamline into the
  data folder, refuses any folder inside the checkout and deletes any DLL whose Authenticode
  signature is not a valid NVIDIA one.
- **Loading at start**: `sl.interposer.dll`, and `sl.common.dll`, `sl.dlss.dll` and
  `nvngx_dlss.dll` where present, are checked offline with `WinVerifyTrust` (no revocation
  lookup, cached URLs only): the signature must be valid and the signer's name must say NVIDIA.
  Then the interposer is loaded from its own folder. The files are checked by path and loaded
  by path, so a file swapped in between would not be caught; the folder is the user's own.
  Streamline uses manual hooking, so nothing of D3D12 or DXGI is hooked. Its downloads (OTA)
  and downloaded plugins stay off. Once the host device exists, it is handed over with
  `slSetD3DDevice`. With the software adapter (`LEONIDA_D3D12_WARP`) DLSS stays off before
  anything is loaded or patched.
- **The title's FidelityFX**: the title's imports from `amd_fidelityfx_loader_xbox.dll` are bound
  through `leonida_hle::modules::fidelityfx`. Everything except an upscale dispatch goes to the
  title's own DLL. An upscale dispatch is recorded as `slEvaluateFeature` at the same place in the
  title's command list. It uses the same colour, depth, motion vectors, exposure and reactive mask,
  and writes to the same output. If DLSS cannot take a frame (also when its evaluation fails
  while recording), the title's own FidelityFX runs for it, as long as the title's context was
  made at the size asked. On the patched Series X image the context is made smaller (the
  shipping FSR4 accepts only 1280x720 to 1920x1080, and at other sizes the title divides by
  zero), so such a frame gets no upscaling.
- **A failed hand-over**: if `slSetD3DDevice` fails or the DLSS functions are missing, the image
  may already be on the FidelityFX path. Nothing then upscales and the picture stops updating.
  The log says so as an error and tells the user to start again without `LEONIDA_DLSS`.
- **Series X renderer**: stock Series X uses its own temporal pass and never loads the upscaler.
  `apps/leonida/src/upscaler_path.rs` switches it to the FidelityFX path (renderer 7, as Series S
  uses) at the mode's scale. Two sites are checked first against a BLAKE3 hash of their stock
  bytes, and two scale constants by value (2/3). Only the hashes are in the repository, not the
  bytes: the one stock instruction the patch keeps (the store of the noted mode) is copied from
  the image at run time, two bytes on. If any check fails, nothing is written and the title runs
  as before. The patched image goes in its own cache folder per mode, so the normal image cache
  stays as it was.

## What was and was not measured

Nothing was run in the game for this change, and no frame times were taken. The `dlss`
profile is opt-in until a tester measures it.

Checked without the game or NVIDIA files: unit tests for the switch parsing, the mode scales and
render sizes, the Streamline structure offsets, that OTA and downloaded plugins stay off, that
missing or unsigned files leave DLSS off, the FidelityFX description layouts, and the site-hash
check on synthetic bytes. An ignored test (`LEONIDA_TEST_IMAGE`) found both site hashes and both
scales matching on one Series X executable (before the store was copied from the image; the
checks themselves did not change). That executable's SHA-256 differs from the build the
reference fork names, so other builds may differ.

Limits and open points:

- The reference fork did not observe `presentCommon()` being called. This port does not need it,
  but nothing here shows whether the title's own present timing suits DLSS.
- The camera constants are approximated (marked `shortcut:` in `plan.rs`): the title names no
  camera matrices, so the projection is built from the sizes, and the clip matrices stay the
  identity.
- Series S was not tried. Its renderer already takes the FidelityFX path, but no site check was
  made for it.
- No RTX card ran this code. Whether the jitter and motion-vector signs are right is unknown until
  a tester runs it.
- When the title destroys its upscaler, DLSS frees its resources once submitted work has run.
  A list recorded with a DLSS evaluation and not yet submitted, or one still held by
  `LEONIDA_ASYNC_SUBMIT`, is not waited for. This assumes the title destroys the context only
  when idle; that is a guess, not seen.
- `slShutdown` is called only by `leonida dlss-check`, not when the game ends.
- Frame generation came later, in [its own note](2026-11-27-frame-generation.md); it needs an
  RTX 40 or 50 tester.

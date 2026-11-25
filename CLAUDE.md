# Guide for coding agents

Read this before changing anything. [TODO.md](TODO.md) is the work list, [docs/asset-policy.md](docs/asset-policy.md)
the rule that matters most, and [docs/architecture.md](docs/architecture.md) how the crates fit together.

## What this is

Leonida Legacy runs the Xbox Series build of Grand Theft Auto VI natively on Windows, from the user's own copy. The
game's x86-64 code runs unchanged inside our process; we supply the console platform under it: the loader, the
runtime services, a Direct3D 12 back end and a shader recompiler (console GPU code to DXIL, on first use, cached).

The goal is a console-like experience on PC: straight into play, a steady 30 fps (33 ms a frame) or better, the
console's picture, no stutter, no crashes.

It is a fork of [fxceless/victory-gta6](https://github.com/fxceless/victory-gta6), itself a fork of
[Marvisak/VIctory](https://github.com/Marvisak/VIctory), renamed to Leonida Legacy. GPL-3.0-or-later.

## Hard rules

- **No game data, ever.** No assets, executables or parts of them, shaders in any form, disassembly, dumps,
  screenshots, keys, or logs that quote any of it, in commits, issues, pull requests or test fixtures. Tests build
  their input with our own code, or read real files from a path in an environment variable and skip when it is unset.
  The one exception is `crates/leonida-formats/src/bcpack-tables.bin`; no new ones.
- **Derived data stays outside the repository.** Anything that writes caches, converted data or logs goes through
  `leonida_core::ensure_outside_repo` (or `Config::cache_subdir` / `data_subdir`, which call it).
- **The repository is text only.** `cargo xtask policy` allows these extensions: `rs toml md txt lock yml yaml json
  hlsl hlsli ps1 cmd sh`, files under 512 KiB, no long hex or base64 runs. No Python files: write helpers in Rust
  (`xtask`) or PowerShell.
- **Keep the credits.** Links, upstream commit and issue references (`upstream VIctory 86a5ed6`, `VIctory #1122`) and
  the README's credits name VIctory and victory-gta6 on purpose. Don't rename them, and don't remove the license or
  copyright notices.
- **Offline by design.** The title must not reach the network.
- **Never touch `leonida.toml`** in a commit. It is the user's own config and git ignores it.

## Before you start

- Pick an item from [TODO.md](TODO.md). Search the open pull requests on
  [ProjectLeonidaL/leonida](https://github.com/ProjectLeonidaL/leonida) for its ID; if someone has it, pick another.
- Claim it by opening a draft pull request with the ID in the title (`P-3: ...`) as soon as you start.
- One item, one pull request. Touch only the files the item needs. The pull request that finishes an item deletes
  its row in `TODO.md` and edits no other row. New work goes in as a new row with the next free ID; never renumber.
- Read the code and [docs/fork-notes.md](docs/fork-notes.md) before guessing: many things were already measured
  there. Quote where a number comes from, or say it is a guess.

## Where things are

| Path | What |
|------|------|
| `apps/leonida` | the `leonida` command: `run`, `config`, `translate`, `inspect`, `census`, `shader-info` |
| `crates/leonida-core` | config (`leonida.toml`, `LEONIDA_*` env), the asset-policy guard, logging |
| `crates/leonida-loader` | prepares, maps, binds and starts the title image; crash reports |
| `crates/leonida-hle` | console APIs: files, memory, audio, input, saves, DirectStorage |
| `crates/leonida-gpu` | D3D12.X objects, command lists and title memory on host D3D12 |
| `crates/leonida-shader` | console shader containers to DXIL |
| `crates/leonida-formats` | texture layouts and detiling, audio codecs, BCPack |
| `crates/leonida-pe`, `-pipeline`, `-game` | PE reading, the converter cache, game-specific formats and build ids |
| `xtask` | `cargo xtask policy` and `cargo xtask install-hooks` |
| `scripts/rename-from-upstream.ps1` | renames code taken from upstream (`victory` to `leonida`) |
| `run-gta6.ps1` | builds and starts the game with the measured frame-rate switches |
| `docs/` | architecture, asset policy, ray tracing, the fork's measurement log |

## Commands

Windows only; the toolchain is pinned in `rust-toolchain.toml`. None of these need the game except the last two.

```powershell
cargo xtask install-hooks                                   # once: asset-policy check on every commit
cargo check --workspace --all-targets --locked
cargo test -p <crate> --locked                              # while you iterate
cargo test --workspace --locked                             # before the pull request
cargo clippy --workspace --all-targets --locked -- -D warnings                 # what CI runs
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings  # CI, diagnostics build
cargo xtask policy
cargo build --release -p leonida; target\release\leonida.exe run --execute    # asks for the game folder once
cargo build --profile diag -p leonida --features diagnostics                   # debugging aids, target\diag
```

If `cargo xtask` fails with "The directory name is invalid" after the folder was moved, run `cargo clean -p xtask`.

## Known red, not yours

`main` already fails CI: 9 clippy errors in `leonida-gpu` (TODO C-1) and the test
`warp_budget_saturates_without_wraparound_and_preserves_draw_fields` (TODO C-2), both inherited from victory-gta6.
Compare against `main` before and after your change; a pull request must not add a new failure or warning. Fixing
those is its own item, not something to fold into another pull request.

## Conventions

- **Formatting:** `rustfmt.toml` sets a width of 100, but the tree isn't formatted yet (TODO C-3). Don't run
  `cargo fmt` on the whole tree; it rewrites almost every file and conflicts with every open pull request. Keep the
  lines you write within 100 columns and formatted the way `rustfmt` would.
- **Match the surrounding code:** its naming, its comment density and its plain-prose comments that say why.
- **Switches:** a new behaviour that might break a picture or lose the device goes behind a `LEONIDA_*` environment
  variable, off by default, described in a comment where it is read and in `docs/fork-notes.md`.
- **`shortcut:`** marks code that approximates what the console does. Add the marker when you approximate; remove it
  when you replace one with the real behaviour.
- **Unsafe:** `unsafe_op_in_unsafe_fn` is denied. Give every new `unsafe` block a `// SAFETY:` comment.
- **Line endings:** `.ps1` and `.cmd` files are CRLF, everything else LF (`.gitattributes`).

## Measuring

- Measure on your own machine before and after, in the same scene, and put both figures in the pull request with
  your CPU, GPU and driver. Most numbers in the docs come from one RTX 4070.
- Post figures, never log files: logs can quote game data.
- The first run after a change to `leonida-shader` re-translates every shader (the cache keys on the translator's
  source), so it stutters. Don't measure that run.

## Upstream

A fresh clone has neither upstream as a remote. Add them once:

```powershell
git remote add upstream https://github.com/fxceless/victory-gta6.git
git remote add victory https://github.com/Marvisak/VIctory.git
```

To bring a fix in:

```powershell
git fetch victory
git cherry-pick <hash>
powershell -ExecutionPolicy Bypass -File scripts\rename-from-upstream.ps1
cargo check --workspace --all-targets --locked
```

One upstream commit (or a few related ones) per pull request, with the upstream hash in the description.

## Commits and pull requests

- Conventional Commits, short and specific: `fix(gpu): ...`, `feat: ...`, `docs: ...`.
- Pull requests go to `ProjectLeonidaL/leonida`, branch `main`. Put the TODO ID in the title, say what you tested
  and on which GPU and driver, and say if it was AI-assisted.

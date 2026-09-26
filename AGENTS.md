# AGENTS.md

## Build

```
nix build . --option builders '' --print-build-logs
```

Do **not** run `cargo build` — `cargo` only exists inside `nix develop` (flake
devShell). `nix build` runs `cargo test` (checkPhase) then
`cargo clippy -- -D warnings` (postCheck) — it is the test/lint gate. GitHub
Actions `build.yml` only checks that all three release platforms compile; it
runs no tests.

```
nix fmt          # format all files via treefmt-nix
nix flake check  # verifies flake evaluation + formatting
```

Run `nix fmt` before every commit.

## Nix source trap

`src = self` in `package.nix` uses `lib.cleanSource`, which **only includes
git-tracked files**. New or renamed source files are invisible to `nix build`
until `git add`ed.

Flake inputs: `nixpkgs/nixpkgs-unstable`, `treefmt-nix`. `treefmt.nix` holds the
formatter config; `flake.nix` only evaluates it.

## Lint

`unwrap()` and `expect()` are **compile errors**
(`unwrap_used`/`expect_used = deny` in `Cargo.toml`). All clippy warnings are
fatal in postCheck. The crate is Rust **edition 2024**.

## Architecture

```
src/main.rs    CLI (clap derive) — entrypoint, imports lib crate
src/lib.rs     Public API: convert_image, convert_video, stream_video, Options, DeviceMode
src/sysex.rs   Roland SC sysex construction — pure Rust, zero deps
src/image.rs   OTSU binarization + ffmpeg pixel extraction + interlace ordering
src/process.rs ffmpeg pipeline -> SMF or real-time MIDI port
```

`sysex.rs` has no external dependencies — safest to modify, easiest to test.

## API gotchas (lib.rs)

- `convert_video(input, &options, &arena)` returns `Smf<'a>` borrowed from a
  caller-owned `midly::Arena` — the arena must outlive the SMF. There is no
  `to_static()`.
- `convert_image` takes tight-packed GRAY8 pixels at the mode's resolution: 256
  (SC-55), 10240 (SC-8850 160x64), or 8192 (SD-90 128x64) bytes.

## CLI quirks (main.rs)

- `--sd90` only applies together with `--sc8850` (Sd90 wins if both set).
- `--edge` takes an optional threshold: bare `--edge` means 50, `--edge 100`
  explicit.
- `--dither` and `--edge` are SC-8850/SD-90 only; `--interlace` is SC-8850 only.
- Auto output path: `{stem}_s[i][e]f{framerate}.mid` (e.g.
  `Bad Apple!! PV_sif10.mid`).

## Features

`midi-output` (default: on) gates the `midir` crate and `--midi-port` /
`--list-ports` CLI options. Code gated with `#[cfg(feature = "midi-output")]`.

## Testing

Tests are `#[cfg(test)] mod tests` blocks in `sysex.rs` (10 tests) and
`image.rs` (10 tests). All pure functions — no fixtures, no ffmpeg, no MIDI
hardware needed. Run via `nix build` (automatic in checkPhase) or `nix develop`
\+ `cargo test`.

## Dependencies

| Crate         | Purpose                                                          |
| ------------- | ---------------------------------------------------------------- |
| ffmpeg-next 9 | Video decode, fps/scale filter (requires `ffmpeg` system lib)    |
| midly 0.5     | MIDI SMF read/write                                              |
| midir 0.10    | Real-time MIDI port output (optional, via `midi-output` feature) |
| clap 4        | CLI argument parsing                                             |

No `image` crate — OTSU thresholding is a ~40-line pure Rust implementation.

## Conventions

- All source characters are ASCII — no Unicode arrows, dashes, or multiplication
  signs.
- Errors use `crate::Result<T>` (`Box<dyn Error>`); string errors via
  `"msg".into()?` / `ok_or(...)?`.
- `ffmpeg::init()` is wrapped in `std::sync::OnceLock` (fn `ensure_ffmpeg`,
  process.rs:18).
- Roland checksum: `(128 - sum % 128) & 0x7F` (not `128 - sum % 128 % 128` —
  precedence bug fixed).

# AGENTS.md

## Build

```
nix build . --option builders '' --print-build-logs
```

Do **not** run `cargo build` — `cargo` is only available inside `nix develop`.  
Build includes `cargo test` + `cargo clippy -- -D warnings` in checkPhase.

## Nix source trap

`src = self` in `package.nix` uses `lib.cleanSource`, which **only includes git-tracked files**.  
New or renamed source files are invisible to `nix build` until `git add`ed.

## Lint

`unwrap()` and `expect()` are **compile errors** (`[lints.clippy]` in `Cargo.toml`).  
All clippy warnings are fatal in CI (`-D warnings` in `postCheck`).  
The crate is Rust **edition 2024**.

## Architecture

```
src/main.rs    CLI (clap) — entrypoint, imports lib crate
src/lib.rs     Public API: convert_image, convert_video, stream_video
src/sysex.rs   Roland SC sysex construction — pure Rust, zero deps
src/image.rs   OTSU binarization + ffmpeg pixel extraction + interlace ordering
src/process.rs ffmpeg pipeline -> SMF or real-time MIDI port
```

`sysex.rs` has no external dependencies — safest to modify, easiest to test.

## Features

`midi-output` (default: on) gates the `midir` crate and `--midi-port` / `--list-ports` CLI options.  
Code gated with `#[cfg(feature = "midi-output")]`.

## Testing

Tests live in `#[cfg(test)] mod tests` blocks in `sysex.rs` (10 tests) and `image.rs` (5 tests).  
All tests are pure functions — no fixtures, no ffmpeg, no MIDI hardware needed.  
Run via `nix build` (automatic in checkPhase) or `nix develop` + `cargo test`.

## Dependencies

| Crate | Purpose |
|-------|---------|
| ffmpeg-next 9 | Video decode, fps/scale filter (requires `ffmpeg` system lib) |
| midly 0.5 | MIDI SMF read/write |
| midir 0.10 | Real-time MIDI port output (optional, via `midi-output` feature) |
| clap 4 | CLI argument parsing |

No `image` crate — OTSU thresholding is a ~40-line pure Rust implementation.

## Conventions

- All source characters are ASCII — no Unicode arrows, dashes, or multiplication signs.
- Code uses `crate::Result<T>` (`Box<dyn Error>`) for error handling.
- `ffmpeg::init()` is wrapped in `std::sync::OnceLock` (called from `run_pipeline`).
- Roland checksum uses `(128 - sum % 128) & 0x7F`, not `128 - sum % 128 % 128` (operator precedence bug fixed).

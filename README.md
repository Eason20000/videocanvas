# VideoCanvas

Convert videos to MIDI for Roland Sound Canvas (SC-55/SC-8850/SD-90).

## CLI

```
videocanvas <video> [options]
```

| Option               | Default | Description                                    |
| -------------------- | ------- | ---------------------------------------------- |
| `-f, --framerate`    | 30      | Output framerate (via ffmpeg fps filter)       |
| `-s, --sc8850`       | off     | SC-8850 mode (160x64 resolution)               |
| `-i, --interlace`    | off     | Interlace (SC-8850 only)                       |
| `-e, --sd90`         | off     | SD-90 reduced columns (SC-8850 only)           |
| `-d, --dither`       | off     | Floyd-Steinberg dithering (SC-8850/SD-90 only) |
| `--edge [THRESHOLD]` | 50      | Sobel edge detection (SC-8850/SD-90 only)      |
| `-o, --output`       | auto    | Output .mid path                               |
| `--midi-port`        | -       | Stream to MIDI output port in real-time        |
| `--list-ports`       | -       | List available MIDI output ports               |

### With Nix

```
nix run . -- <video> [options]
nix build && ./result/bin/videocanvas <video> [options]
nix develop  # then: cargo build && cargo run -- <video> [options]
```

### Prebuilt releases

Download from the GitHub Releases page (Windows portable zip, Linux AppImage,
macOS tarball). Binaries are unsigned: on macOS run
`xattr -d com.apple.quarantine videocanvas` after extracting; on Windows confirm
the SmartScreen prompt.

## Library

```rust
use videocanvas::{convert_image, convert_video, Arena, Options, DeviceMode};

// single image -> Roland sysex messages
let sysex = convert_image(&gray8_pixels, DeviceMode::Sc8850);

// video -> MIDI file
let arena = Arena::new();
let smf = convert_video("input.mp4", &Options {
    framerate: 24,
    mode: DeviceMode::Sc8850,
    interlace: true,
    dither: false,
    edge: None,
}, &arena)?;
smf.save("output.mid")?;
```

## License

GPL-3.0

# VideoCanvas
Convert videos to MIDI files for Roland Sound Canvas (with SC-8850 support).

## Usage

### With Nix

```bash
nix run . -- <video-file> [options]
nix build && ./result/bin/videocanvas <video-file> [options]
nix develop  # then: python main.py <video-file> [options]
```

### Without Nix

```bash
pip install .
videocanvas <video-file> [options]
```

Run `videocanvas -h` to see all options.

## License
GPL-3.0

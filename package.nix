{ self, lib, rustPlatform, pkg-config, clang, ffmpeg, alsa-lib, clippy }:

rustPlatform.buildRustPackage rec {
  pname = (lib.importTOML (src + "/Cargo.toml")).package.name;
  version = (lib.importTOML (src + "/Cargo.toml")).package.version;

  src = self;
  cargoLock.lockFile = src + "/Cargo.lock";

  nativeBuildInputs = [ pkg-config clang rustPlatform.bindgenHook clippy ];
  buildInputs = [ ffmpeg alsa-lib ];

  env.FFMPEG_DIR = ffmpeg.dev;

  doCheck = true;

  postCheck = ''
    cargo clippy --profile release --offline -- -D warnings
  '';
}

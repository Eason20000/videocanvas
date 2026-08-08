{

  description = "Convert videos to MIDI for Roland Sound Canvas";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }: 
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages.${system}.default = pkgs.callPackage ./package.nix { inherit self; };
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          rustc
          cargo
          pkg-config
          clang
          ffmpeg
          alsa-lib
        ];
        env.FFMPEG_DIR = pkgs.ffmpeg.dev;
      };
    };

}

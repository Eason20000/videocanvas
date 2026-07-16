{
  description = "Convert videos to MIDI for Roland Sound Canvas";
  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
  outputs = { self, nixpkgs }:
    let
      eachSystem = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ];
    in {
      packages = eachSystem (system: let
        pkgs = import nixpkgs { inherit system; };
        python = pkgs.python314;
      in {
        default = python.pkgs.buildPythonApplication {
          pname = "videocanvas";
          version = "0.4.0";
          pyproject = true;
          src = self;
          build-system = with python.pkgs; [ setuptools ];
          dependencies = with python.pkgs; [ opencv4 mido ];
          pythonRemoveDeps = [ "opencv-python" ];
          doCheck = false;
          meta.license = pkgs.lib.licenses.gpl3Only;
        };
      });
      devShells = eachSystem (system: let
        pkgs = import nixpkgs { inherit system; };
      in {
        default = pkgs.mkShell {
          packages = [ (pkgs.python314.withPackages (ps: [ ps.opencv4 ps.mido ])) ];
        };
      });
    };
}

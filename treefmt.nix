{ pkgs, ... }:
{
  projectRootFile = "flake.nix";
  programs.nixfmt.enable = true;
  programs.rustfmt.enable = true;
  programs.shfmt.enable = true;
  programs.yamlfmt.enable = true;
  programs.actionlint.enable = true;
  programs.mdformat = {
    enable = true;
    settings.wrap = 80;
    plugins = ps: [ ps.mdformat-gfm ];
  };
  settings.formatter.shfmt.includes = [
    "*.sh"
    "*.bash"
    "packaging/appimage/AppRun"
  ];
}

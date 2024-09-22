{ pkgs ? import <nixpkgs> { } }:

let
  libDeps = with pkgs; [
    # gui examples (minifb)
    libxkbcommon
    xorg.libXcursor
    xorg.libX11
  ];
in
pkgs.mkShell {
  packages = with pkgs; [
    # Base deps
    alsa-lib
    pkg-config

    # benchmarks
    gnuplot

    # Development
    nixpkgs-fmt
    rustup
  ] ++ libDeps;

  LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libDeps}";
}

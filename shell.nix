{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell rec {
  packages = with pkgs; [
    # Base deps
    alsa-lib
    pkg-config

    # gui examples (minifb)
    libxkbcommon
    xorg.libXcursor
    xorg.libX11

    # Development
    nixpkgs-fmt
    rustup
  ];
}

{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell rec {
  packages = with pkgs; [
    alsa-lib
    pkg-config
  ];
}

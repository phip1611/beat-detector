{
  pkgs ? import <nixpkgs> { },
}:

let
  # Runtime dependencies for GUIs and graphics generation.
  # Needed for examples and tests.
  libDeps = with pkgs; [
    fontconfig
    libxkbcommon
    xorg.libXcursor
    xorg.libX11
  ];
in
pkgs.mkShell {
  packages =
    with pkgs;
    [
      # Base deps
      alsa-lib
      pkg-config

      # benchmarks
      gnuplot

      # Development
      nixfmt-rfc-style
      rustup
    ]
    ++ libDeps;

  LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libDeps}";
}

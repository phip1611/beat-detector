{
  description = "beat-detector";

  inputs = {
    # We follow the latest stable release of nixpkgs
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
  };

  outputs =
    inputs@{ self, nixpkgs, ... }:
    let
      # We just use "every system" here to not restrict any user. However, it
      # likely happens that certain packages don't build for/under certain
      # systems.
      systems = nixpkgs.lib.systems.flakeExposed;
      forAllSystems =
        function: nixpkgs.lib.genAttrs systems (system: function nixpkgs.legacyPackages.${system});
    in
    {
      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
      devShells = forAllSystems (pkgs: {
        default = import ./shell.nix {
          inherit pkgs;
        };
      });
    };
}

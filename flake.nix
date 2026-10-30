{
  description = "NixOS Advanced Rebuild Flake - narf CLI and narf-tui";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default-linux";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = import inputs.systems;

      imports = [./packages];

      perSystem = {self', ...}: {
        packages = {
          default = self'.packages.narf;
          narf-tui =
            self'.packages.narf
            // {
              meta = self'.packages.narf.meta // {mainProgram = "narf-tui";};
            };
        };
      };

      flake.overlays.default = final: _prev: {
        inherit (inputs.self.packages.${final.stdenv.hostPlatform.system}) narf narf-tui;
      };
    };
}

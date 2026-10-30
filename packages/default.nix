{
  perSystem = {
    lib,
    pkgs,
    ...
  }: let
    root = ./.;
    workspace = lib.importTOML (root + "/Cargo.toml");
    manifest = lib.importTOML (root + "/narf/Cargo.toml");
    crate = name: [
      (root + "/${name}/Cargo.toml")
      (root + "/${name}/src")
    ];
  in {
    # Builds the whole workspace: the `narf` CLI and `narf-tui`.
    packages.narf = pkgs.rustPlatform.buildRustPackage {
      pname = manifest.package.name;
      inherit (workspace.workspace.package) version;

      src = lib.fileset.toSource {
        inherit root;
        fileset = lib.fileset.unions (
          [
            (root + "/Cargo.toml")
            (root + "/Cargo.lock")
          ]
          ++ crate "narf"
          ++ crate "narf-tui"
        );
      };
      cargoLock.lockFile = root + "/Cargo.lock";

      nativeBuildInputs = [pkgs.makeWrapper];

      postInstall = ''
        for program in $out/bin/narf $out/bin/narf-tui; do
          wrapProgram "$program" \
            --prefix PATH : ${lib.makeBinPath [pkgs.git pkgs.nh]} \
            --suffix PATH : ${lib.makeBinPath [pkgs.nix pkgs.nixos-rebuild]}
        done
      '';

      meta = {
        inherit (manifest.package) description;
        mainProgram = "narf";
      };
    };
  };
}

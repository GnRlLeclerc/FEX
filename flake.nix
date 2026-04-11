{
  description = "Fancy and minimal file explorer build with Slint";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        src =
          let
            filterSlintFiles = path: _type: builtins.match ".*slint$" path != null;
            cargoOrSlint = path: type: (filterSlintFiles path type) || (craneLib.filterCargoSources path type);
          in
          pkgs.lib.cleanSourceWith {
            src = ./.;
            name = "source";
            filter = cargoOrSlint;
          };

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            pkg-config
            autoPatchelfHook
          ];
          buildInputs = with pkgs; [
            libGL
            libxkbcommon
            wayland
            fontconfig
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        fex = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        packages.default = fex;
        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            slint-lsp
            pkg-config
          ];
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath commonArgs.buildInputs;
          PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
            commonArgs.buildInputs
            ++ [
              pkgs.freetype
            ]
          );
        };
      }
    );
}

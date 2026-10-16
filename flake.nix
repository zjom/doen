{
  description = "doen — cli to get shit doen";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        doen = pkgs.callPackage ./package.nix { };
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.doen;
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.doen;
        };
      });

      checks = forAllSystems (pkgs: {
        doen = self.packages.${pkgs.stdenv.hostPlatform.system}.doen;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          # doCheck = false keeps nativeCheckInputs (git) out, so the user's own git stays on PATH
          inputsFrom = [
            (self.packages.${pkgs.stdenv.hostPlatform.system}.doen.overrideAttrs { doCheck = false; })
          ];
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            cargo-edit
            cargo-watch
            cargo-release
            nixfmt
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-tree);
    };
}

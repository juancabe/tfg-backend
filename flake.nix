{
  description = "Rust Dev Environment with Diesel and Embedded Postgres";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        pgPackage = pkgs.postgresql_16;

        pgSetupScript = import ./nix/postgres-setup-script.nix {
          inherit pkgs pgPackage;
        };

        rustApp = pkgs.rustPlatform.buildRustPackage {
          pname = "backend";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl pgPackage ];
          doCheck = false;
          postInstall = ''
            mkdir -p $out/share
            cp -r migrations $out/share/migrations
          '';
        };

        devShell = import ./nix/dev.nix {
          inherit pkgs pgPackage pgSetupScript;
        };
      in
      {
        packages = {
          default = rustApp;
        };

        devShells.default = devShell;
      }
    ) // {
      homeManagerModules.default = import ./nix/home-manager-module.nix { inherit self; };
    };
}

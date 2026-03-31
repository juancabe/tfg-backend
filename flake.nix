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

        pgSetupScript = pkgs.writeShellScriptBin "setup-postgres" ''
          export PGDATA="''${PGDATA:-$PWD/.nix-pg/data}"
          export PGHOST="''${PGHOST:-$PWD/.nix-pg/run}"
          export PGLOG="''${PGLOG:-$PWD/.nix-pg/postgres.log}"

          mkdir -p "$PGHOST"

          if [ ! -d "$PGDATA" ]; then
            ${pgPackage}/bin/initdb -D "$PGDATA" --no-locale --encoding=UTF8 --auth=trust -U postgres > /dev/null
          fi

          if ! ${pgPackage}/bin/pg_ctl status -D "$PGDATA" > /dev/null 2>&1; then
             ${pgPackage}/bin/pg_ctl start -D "$PGDATA" -l "$PGLOG" -o "-k $PGHOST"
             until ${pgPackage}/bin/pg_isready -h "$PGHOST"; do sleep 1; done
             if ! ${pgPackage}/bin/psql -h "$PGHOST" -U postgres -lqt | cut -d \| -f 1 | grep -qw "postgres"; then
               ${pgPackage}/bin/createdb -h "$PGHOST" -U postgres postgres
             fi
          fi
        '';

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
          inherit pkgs pgPackage; 
        };
      in
      {
        packages = {
          default = rustApp;
          setup-postgres = pgSetupScript;
        };

        devShells.default = devShell;
      }
    ) // {
      homeManagerModules.default = import ./nix/home-manager-module.nix { inherit self; };
    };
}

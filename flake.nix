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

        # Script to initialize/start Postgres
        pgSetupScript = pkgs.writeShellScriptBin "setup-postgres" ''
          export PGDATA=$PWD/.nix-pg/data
          export PGHOST=$PWD/.nix-pg/run
          export PGLOG=$PWD/.nix-pg/postgres.log

          mkdir -p $PGHOST

          if [ ! -d "$PGDATA" ]; then
            echo "Initializing postgres data directory..."
            ${pgPackage}/bin/initdb -D "$PGDATA" --no-locale --encoding=UTF8 --auth=trust -U postgres > /dev/null
          fi

          if ! ${pgPackage}/bin/pg_ctl status -D "$PGDATA" > /dev/null 2>&1; then
             echo "Starting PostgreSQL..."
             ${pgPackage}/bin/pg_ctl start -D "$PGDATA" -l "$PGLOG" -o "-k $PGHOST"
             
             until ${pgPackage}/bin/pg_isready -h "$PGHOST"; do
               sleep 1
             done

             if ! ${pgPackage}/bin/psql -h "$PGHOST" -U postgres -lqt | cut -d \| -f 1 | grep -qw "postgres"; then
               ${pgPackage}/bin/createdb -h "$PGHOST" -U postgres postgres
             fi
             
             echo "Postgres is running on host $PGHOST"
          else
             echo "Postgres is already running."
          fi
        '';

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            openssl
            pkg-config
            pgPackage
            diesel-cli
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })
            pgSetupScript 
          ];

          shellHook = ''
            export PGDATA=$PWD/.nix-pg/data
            export PGHOST=$PWD/.nix-pg/run
            
            setup-postgres
            
            # --- FIX IS HERE ---
            # We use 'localhost' to satisfy the URI parser, but ?host overrides it 
            # to use the unix socket in the directory specified by $PGHOST
            export DATABASE_URL="postgres://postgres@localhost/postgres?host=$PGHOST"

            trap "'${pgPackage}/bin/pg_ctl' stop -D '$PGDATA' -m fast" EXIT

            echo "Environment ready."
          '';
        };
      }
    );
}

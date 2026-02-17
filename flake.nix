{
  description = "Rust Dev Environment and Systemd Service";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    let
      moduleOutputs = {
        nixosModules.default = { config, lib, pkgs, ... }:
          let
            cfg = config.services.my-rust-service;
          in
          {
            options.services.my-rust-service = {
              enable = lib.mkEnableOption "enable my rust service";

              databaseUrl = lib.mkOption {
                type = lib.types.str;
                description = "Connection string for the database.";
                example = "postgres://user:password@localhost/mydb";
              };

              logLevel = lib.mkOption {
                type = lib.types.str;
                default = "info";
                description = "RUST_LOG verbosity level.";
              };
            };

            config = lib.mkIf cfg.enable {
              systemd.services.my-rust-service = {
                wantedBy = [ "multi-user.target" ];
                after = [ "network.target" "postgresql.service" ];
                
                environment = {
                  DATABASE_URL = cfg.databaseUrl;
                  RUST_LOG = cfg.logLevel;
                };

                serviceConfig = {
                  ExecStart = "${self.packages.${pkgs.system}.default}/bin/backend";
                  Restart = "always";
                  
                  DynamicUser = true;
                  ProtectHome = true;
                  NoNewPrivileges = true;
                };
              };
            };
          };
      };

      systemOutputs = flake-utils.lib.eachDefaultSystem (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };

          rustPackage = pkgs.rustPlatform.buildRustPackage {
            pname = "my-app-binary-name"; 
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };

          pgPackage = pkgs.postgresql_16;
          
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
             fi
          '';

        in
        {
          packages.default = rustPackage;

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
              export DATABASE_URL="postgres://postgres@localhost/postgres?host=$PGHOST"
              trap "'${pgPackage}/bin/pg_ctl' stop -D '$PGDATA' -m fast" EXIT
              export RUST_LOG=debug
              echo "Environment ready."
            '';
          };
        }
      );
    in
    moduleOutputs // systemOutputs;
}

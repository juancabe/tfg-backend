{
  description = "Rust Dev Environment with Diesel and Embedded Postgres";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    # 1. Define system-specific outputs (packages, devShells)
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        pgPackage = pkgs.postgresql_16;

        # Modified script: Respects injected environment variables (like from systemd),
        # but falls back to $PWD for `nix develop` use.
        pgSetupScript = pkgs.writeShellScriptBin "setup-postgres" ''
          export PGDATA="''${PGDATA:-$PWD/.nix-pg/data}"
          export PGHOST="''${PGHOST:-$PWD/.nix-pg/run}"
          export PGLOG="''${PGLOG:-$PWD/.nix-pg/postgres.log}"

          mkdir -p "$PGHOST"

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

        # Add your Rust application package so the Home Manager module can use it
        rustApp = pkgs.rustPlatform.buildRustPackage {
          pname = "backend";
          version = "0.1.0";
          src = ./.;
          cargoLock = { lockFile = ./Cargo.lock; };
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl pgPackage ];
          doCheck = false;
        };

      in
      {
        packages = {
          default = rustApp;
          setup-postgres = pgSetupScript;
        };

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
    ) // {
      # 2. Define system-agnostic outputs (Home Manager Modules)
      homeManagerModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.tfg-backend;

          caddyfile = pkgs.writeText "Caddyfile" ''
            {
                http_port 7777
                https_port 7778
            }

            ${cfg.domain} {
                # Proxy to your Rust backend (assuming it listens on 8080)
                reverse_proxy 127.0.0.1:8080
            }
          '';
        in {
          options.services.tfg-backend = {
            enable = lib.mkEnableOption "Rust Dev environment service";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              description = "The Rust application package to run.";
            };

            setupScript = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.setup-postgres;
              description = "The postgres initialization script.";
            };

            # Add the new option for your secret file
            jwtKeyFile = lib.mkOption {
              type = lib.types.str;
              description = "Path to a file containing the JWT_KEY_BASE64 environment variable.";
              example = "%h/.config/my-app/secrets.env";
            };

            domain = lib.mkOption {
              type = lib.types.str;
              default = "localhost";
              description = "The domain name for Caddy to serve and enable TLS";
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.user.services.caddy = {
              Unit = {
                Description = "Caddy Reverse Proxy";
                After = [ "network.target" ];
              };

              Service = {
                ExecStart = "${pkgs.caddy}/bin/caddy run --config ${caddyfile} --adapter caddyfile";
                ExecReload = "${pkgs.caddy}/bin/caddy reload --config ${caddyfile} --adapter caddyfile";
                Restart = "always";
                
                StateDirectory = "caddy";
              };

              Install = {
                WantedBy = [ "default.target" ];
              };
            };
            systemd.user.services.tfg-backend = {
              Unit = {
                Description = "Rust Dev Environment with Embedded Postgres";
                After = [ "network.target" ];
              };

              Service = {
                # Your standard, non-secret environment variables
                Environment = [
                  "RUST_LOG=debug"
                  "PGDATA=%S/tfg-backend/pg/data"
                  "PGHOST=%t/tfg-backend/pg/run"
                  "PGLOG=%S/tfg-backend/pg/postgres.log"
                  "DATABASE_URL=postgres://postgres@localhost/postgres?host=%t/tfg-backend/pg/run"
                ];

                # Load secret environment variables dynamically at runtime
                EnvironmentFile = cfg.jwtKeyFile;

                StateDirectory = "tfg-backend";
                RuntimeDirectory = "tfg-backend";

                ExecStartPre = "${cfg.setupScript}/bin/setup-postgres";
                ExecStart = "${cfg.package}/bin/main";
                ExecStopPost = "${pkgs.postgresql_16}/bin/pg_ctl stop -D %S/tfg-backend/pg/data -m fast";
                Restart = "always";
              };

              Install = {
                WantedBy = [ "default.target" ];
              };
            };
          };
        };
    };
}

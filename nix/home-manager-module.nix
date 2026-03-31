{ self }:
{ config, lib, pkgs, ... }:
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
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The Rust application package to run.";
    };

    setupScript = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.setup-postgres;
      description = "The postgres initialization script.";
    };

    envFile = lib.mkOption {
      type = lib.types.str;
      description = "Path to a file containing environment variables for the service.";
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
        Environment = [
          "RUST_LOG=debug"
          "PGDATA=%S/tfg-backend/pg/data"
          "PGHOST=%t/tfg-backend/pg/run"
          "PGLOG=%S/tfg-backend/pg/postgres.log"
          "DATABASE_URL=postgres://postgres@localhost/postgres?host=%t/tfg-backend/pg/run"
        ];

        EnvironmentFile = cfg.envFile;

        StateDirectory = "tfg-backend";
        RuntimeDirectory = "tfg-backend";

        ExecStartPre = [
          "${cfg.setupScript}/bin/setup-postgres"
          "${pkgs.diesel-cli}/bin/diesel migration run --migration-dir ${cfg.package}/share/migrations"
        ];
        ExecStart = "${cfg.package}/bin/main";
        ExecStopPost = "${pkgs.postgresql_16}/bin/pg_ctl stop -D %S/tfg-backend/pg/data -m fast";
        Restart = "always";
      };

      Install = {
        WantedBy = [ "default.target" ];
      };
    };
  };
}

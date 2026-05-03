{ self }:
{ config, lib, pkgs, ... }:
let
  cfg = config.services.tfg-backend;
  pgPackage = pkgs.postgresql_16;
  pgSetupScript = import ./postgres-setup-script.nix {
    inherit pkgs pgPackage;
  };
in {
  options.services.tfg-backend = {
    enable = lib.mkEnableOption "Rust Dev environment service";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "The Rust application package to run.";
    };

    envFile = lib.mkOption {
      type = lib.types.str;
      description = "Path to a file containing environment variables for the service.";
      example = "%h/.config/my-app/secrets.env";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.tfg-postgres = {
      Unit = {
        Description = "Embedded PostgreSQL for tfg-backend";
        After = [ "network.target" ];
      };

      Service = {
        Type = "oneshot";
        RemainAfterExit = true;
        Environment = [
          "PGDATA=%S/tfg-backend/pg/data"
          "PGHOST=%t/tfg-backend/pg/run"
          "PGLOG=%S/tfg-backend/pg/postgres.log"
        ];
        StateDirectory = "tfg-backend";
        RuntimeDirectory = "tfg-backend";
        ExecStart = "${pgSetupScript}/bin/setup-postgres";
        ExecStop = "${pgPackage}/bin/pg_ctl stop -D %S/tfg-backend/pg/data -m fast";
      };

      Install = {
        WantedBy = [ "default.target" ];
      };
    };

    systemd.user.services.tfg-backend = {
      Unit = {
        Description = "Rust Dev Environment with Embedded Postgres";
        After = [ "network.target" "tfg-postgres.service" ];
        Requires = [ "tfg-postgres.service" ];
      };

      Service = {
        Environment = [
          "RUST_LOG=debug"
          "PGHOST=%t/tfg-backend/pg/run"
          "DATABASE_URL=postgres://postgres@localhost/postgres?host=%t/tfg-backend/pg/run"
        ];

        EnvironmentFile = cfg.envFile;

        ExecStart = "${cfg.package}/bin/main";
        Restart = "always";
      };

      Install = {
        WantedBy = [ "default.target" ];
      };
    };
  };
}

{ pkgs, pgPackage, pgSetupScript }:

let
  pgBin = "${pgPackage}/bin";
in

pkgs.mkShell {
  buildInputs = with pkgs; [
    openssl
    pkg-config
    pgPackage
    diesel-cli
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" "rust-analyzer" ];
    })
  ];

  shellHook = ''
    export PATH="${pgBin}:$PATH"
    export PGDATA=$PWD/.nix-pg/data
    export PGHOST=$PWD/.nix-pg/run
    export PGLOG=$PWD/.nix-pg/postgres.log
    export DATABASE_URL="postgres://postgres@localhost/postgres?host=$PGHOST"
    export RUST_LOG=debug

    ${pgSetupScript}/bin/setup-postgres

    trap 'pg_ctl stop -D $PGDATA -m fast' EXIT
    echo "Environment ready."
  '';
}

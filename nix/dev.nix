{ pkgs, pgPackage }:

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
    
    mkdir -p "$PGHOST"
    
    if [ ! -d "$PGDATA" ]; then
      echo "Initializing postgres data directory..."
      initdb -D "$PGDATA" --no-locale --encoding=UTF8 --auth=trust -U postgres > /dev/null
    fi
    
    if ! pg_ctl status -D "$PGDATA" > /dev/null 2>&1; then
      echo "Starting PostgreSQL..."
      pg_ctl start -D "$PGDATA" -l "$PGLOG" -o "-k $PGHOST"
      until pg_isready -h "$PGHOST"; do sleep 1; done
      if ! psql -h "$PGHOST" -U postgres -lqt | cut -d \| -f 1 | grep -qw "postgres"; then
        createdb -h "$PGHOST" -U postgres postgres
      fi
      echo "Postgres is running on host $PGHOST"
    else
      echo "Postgres is already running."
    fi
    
    trap 'pg_ctl stop -D $PGDATA -m fast' EXIT
    echo "Environment ready."
  '';
}
{ pkgs, pgPackage }:

pkgs.writeShellScriptBin "setup-postgres" ''
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
''

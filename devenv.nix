{ pkgs, ... }:
{
  packages = with pkgs; [
    sqlx-cli
    just
    git
  ];

  languages = {
    rust = {
      enable = true;
      channel = "nightly";
      version = "latest";
      mold.enable = true;
    };
    javascript = {
      enable = true;
      pnpm.enable = true;
    };
  };

  services.postgres = {
    enable = true;
    package = pkgs.postgresql_18;
    initialDatabases = [
      {
        name = "nexigon";
        user = "nexigon";
        pass = "nexigon";
      }
    ];
    initialScript = "CREATE ROLE nexigon SUPERUSER;";
    listen_addresses = "127.0.0.1";
  };

  env.DATABASE_URL = "postgres://nexigon:nexigon@127.0.0.1/nexigon";
}

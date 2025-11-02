{
  pkgs,
  rustPlatform,
  lib,
  ...
}:
rustPlatform.buildRustPackage rec {
  pname = "antares-rpc-client";
  version = "0.1.0";
  src = builtins.path {
    name = pname;
    path = ./.;
  };
  cargoHash = "sha256-7cnuZuCCxapfLjNKrUjTIXr7k++1Pe+eH+yH06cZJYM=";
}

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
  cargoHash = "sha256-XhtZZ32vlmh1B8XVDG0bnyB8rs5yoiL5rpvAl2r8d/M=";
}

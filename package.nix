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
  cargoHash = "sha256-kl6Dy/IECqJ8Vd1vNt2+bun6TpLdlgw4VTCJyqwLFrs=";
}

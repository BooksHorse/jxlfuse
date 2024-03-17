{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }:
  let pkgs = nixpkgs.legacyPackages.x86_64-linux;
      libjxl = pkgs.libjxl.overrideAttrs (oldAttrs: rec {
    inherit (oldAttrs) pname;
    version = "0.10.2";
  src = pkgs.fetchFromGitHub {
    owner = "libjxl";
    repo = "libjxl";
    rev = "v${version}";
    hash = "sha256-n5KNbbw6NQRROEM7Cojla/igRCFNawUq7nfhzJlMlPI=";
    # There are various submodules in `third_party/`.
    fetchSubmodules = true;
  };
  });
  in {

    packages.x86_64-linux.hello = nixpkgs.legacyPackages.x86_64-linux.hello;

    packages.x86_64-linux.default = self.packages.x86_64-linux.hello;
    devShells.x86_64-linux.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        fuse
        pkg-config
        libjxl
        cmake
      ];
      shellHook = ''
      export DEP_JXL_LIB=${pkgs.libjxl}/lib
      '';
    };
  };
}

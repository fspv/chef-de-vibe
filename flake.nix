{
  description = "Chef de Vibe - A Rust application with embedded React frontend";
  
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" ];
        };
        
        # Frontend build using buildNpmPackage
        frontend = pkgs.buildNpmPackage {
          pname = "chef-de-vibe-frontend";
          version = "0.2.6";
          
          src = ./frontend;
          
          # This hash will need to be updated - build once with wrong hash to get correct one
          npmDepsHash = "sha256-7Pi0D4z+AXwu5UuB20FCQeu+LOh17Gym2aHGPN31GmI=";
          
          # Use nodejs 22
          nodejs = pkgs.nodejs_22;
          
          # Build the frontend
          buildPhase = ''
            runHook preBuild
            npm run build
            runHook postBuild
          '';
          
          installPhase = ''
            runHook preInstall
            mkdir -p $out
            cp -r dist/* $out/
            runHook postInstall
          '';
          
          # Don't run npm install in the install phase
          dontNpmInstall = true;
        };
        
        # Rust application with embedded frontend
        chef-de-vibe = pkgs.rustPlatform.buildRustPackage {
          pname = "chef-de-vibe";
          version = "0.2.6";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
          ];
          
          buildInputs = with pkgs; [
            openssl
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
          
          # Copy frontend dist to the expected location before building
          preBuild = ''
            mkdir -p frontend/dist
            cp -r ${frontend}/* frontend/dist/
          '';
          
          # Skip tests during build (they can be run separately)
          doCheck = false;
          
          meta = with pkgs.lib; {
            description = "Chef de Vibe - A Rust application with embedded React frontend";
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      in
      {
        packages = {
          default = chef-de-vibe;
          chef-de-vibe = chef-de-vibe;
          frontend = frontend;
        };
        
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            nodejs_22
            nodePackages.npm
            pkg-config
            openssl
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];
          
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
        
        apps.default = {
          type = "app";
          program = "${chef-de-vibe}/bin/chef-de-vibe";
        };
      });
}

{
  description = "Declare an HPS<->FPGA register boundary once, as facet types";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # nixpkgs' rustc only ships the host std; fenix gives us the aarch64 one so
    # the HPS binary cross-builds on saturn without compiling a toolchain.
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      fx = fenix.packages.${system};
      hpsTarget = "aarch64-unknown-linux-musl";
      toolchain = fx.combine [
        fx.stable.cargo
        fx.stable.rustc
        fx.stable.clippy
        fx.stable.rustfmt
        fx.targets.${hpsTarget}.stable.rust-std
      ];
      platform = pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };
    in
    {
      # Static musl, linked by rust-lld against the self-contained crt in
      # rust-std, so it runs on mercury with no libc dependency at all. Plain
      # mkDerivation because buildRustPackage pins --target to the host.
      packages.${system}.blinky-hps = pkgs.stdenv.mkDerivation {
        pname = "blinky";
        version = "0.1.0";
        src = ./.;
        cargoDeps = platform.importCargoLock { lockFile = ./Cargo.lock; };
        nativeBuildInputs = [ toolchain platform.cargoSetupHook ];
        buildPhase = ''
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
            cargo build --release --offline --target ${hpsTarget} --bin blinky
        '';
        installPhase = ''
          install -Dm755 target/${hpsTarget}/release/blinky $out/bin/blinky
        '';
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [ toolchain pkgs.rust-analyzer pkgs.ghdl ];
      };
    };
}

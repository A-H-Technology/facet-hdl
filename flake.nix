{
  description = "Declare an HPS<->FPGA register boundary once, as facet types";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # nixpkgs' rustc only ships the host std; fenix gives us the boards' ones
    # so the HPS binaries cross-build without compiling a toolchain.
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
      # mercury (DE25-Nano, Cortex-A55) and venus (Zybo, Cortex-A9).
      boardTargets = [ "aarch64-unknown-linux-musl" "armv7-unknown-linux-musleabihf" ];
      toolchain = fx.combine ([
        fx.stable.cargo
        fx.stable.rustc
        fx.stable.clippy
        fx.stable.rustfmt
      ] ++ map (t: fx.targets.${t}.stable.rust-std) boardTargets);
      platform = pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };
      # Static musl, linked by rust-lld against the self-contained crt in
      # rust-std, so it runs on the board with no libc dependency at all.
      # Plain mkDerivation because buildRustPackage pins --target to the host.
      boardHello = target: pkgs.stdenv.mkDerivation {
        pname = "hello";
        version = "0.1.0";
        src = ./.;
        cargoDeps = platform.importCargoLock {
          lockFile = ./Cargo.lock;
          # Spade's compiler is a git dependency (not on crates.io past 0.16);
          # Cargo.lock already pins its commit, so fetch by that instead of
          # keeping a second hash in step with every bump.
          allowBuiltinFetchGit = true;
        };
        nativeBuildInputs = [ toolchain platform.cargoSetupHook ];
        buildPhase = ''
          export CARGO_TARGET_${pkgs.lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] target)}_LINKER=rust-lld
          cargo build --release --offline --target ${target} --bin hello --bin hello-async
        '';
        installPhase = ''
          install -Dm755 target/${target}/release/hello $out/bin/hello
          install -Dm755 target/${target}/release/hello-async $out/bin/hello-async
        '';
      };
    in
    {
      packages.${system} = {
        hello-hps = boardHello "aarch64-unknown-linux-musl";
        hello-zybo = boardHello "armv7-unknown-linux-musleabihf";
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [ toolchain pkgs.rust-analyzer pkgs.iverilog ];
      };
    };
}

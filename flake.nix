{
  description = "Declare an HPS<->FPGA register boundary once, as facet types";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc clippy rustfmt rust-analyzer ghdl ];
      };
    };
}

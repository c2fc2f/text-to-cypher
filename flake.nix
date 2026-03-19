{
  description = "A comparative study of two query generation architectures for Neo4j: 1) Direct Translation (Natural Language directly to Cypher) and 2) Staged Translation (Natural Language to CNL/SQUALL/Sparklis, then to Cypher via a fine-tuned model)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default-linux";
  };

  outputs =
    {
      self,
      nixpkgs,
      systems,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      eachSystem = lib.genAttrs (import systems);

      pkgsFor = eachSystem (
        system:
        import nixpkgs {
          localSystem = system;
        }
      );
    in
    {
      packages = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = self.packages.${system}.text-to-cypher;

          text-to-cypher = pkgs.callPackage ./nix/package.nix {
            version = self.rev or self.dirtyRev or "dirty";
          };
        }
      );

      defaultPackage = eachSystem (system: self.packages.${system}.default);

      devShells = eachSystem (system: {
        default =
          pkgsFor.${system}.mkShell.override
            {
              inherit (self.packages.${system}.default) stdenv;
            }
            {
              env = {
                # Required by rust-analyzer
                RUST_SRC_PATH = "${pkgsFor.${system}.rustPlatform.rustLibSrc}";
              };

              nativeBuildInputs = with pkgsFor.${system}; [
                cargo
                rustc
                rust-analyzer
                rustfmt
                clippy

                pkg-config
              ];

              buildInputs = with pkgsFor.${system}; [
                openssl
              ];
            };
      });
    };
}

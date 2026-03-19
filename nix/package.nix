{
  version,
  lib,
  installShellFiles,
  rustPlatform,
  pkg-config,
  buildFeatures ? [ ],
  openssl,
}:

rustPlatform.buildRustPackage {
  pname = "text-to-cypher";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../. (
      # don't include in build
      lib.fileset.unions [
        ../README.md
        ../LICENSE
      ]
    );
  };

  inherit buildFeatures;
  inherit version;

  # inject version from nix into the build
  env.NIX_RELEASE_VERSION = version;

  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [
    installShellFiles
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  meta = with lib; {
    description = "A comparative study of two query generation architectures for Neo4j: 1) Direct Translation (Natural Language directly to Cypher) and 2) Staged Translation (Natural Language to CNL/SQUALL/Sparklis, then to Cypher via a fine-tuned model)";
    homepage = "https://github.com/c2fc2f/text-to-cypher";
    license = licenses.mit;
    maintainers = [ maintainers.c2fc2f ];
  };
}

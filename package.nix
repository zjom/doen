{
  lib,
  rustPlatform,
  installShellFiles,
  git,
}:

let
  cargoToml = lib.importTOML ./Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./src
      ./tests
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];

  # the integration tests shell out to git
  nativeCheckInputs = [ git ];

  postInstall = ''
    installShellCompletion --cmd doen \
      --bash src/completion/doen.bash \
      --zsh src/completion/_doen \
      --fish src/completion/doen.fish
  '';

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.mit;
    mainProgram = "doen";
  };
}

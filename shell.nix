  { pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      zlib.out
      rustup
      nasm
      xorriso
      entr
      llvmPackages.lld
    ];
    shellHook = ''
      export HISTFILE=${toString ./.history}
      export PATH=$PATH:~/.cargo/bin
      '';
  }

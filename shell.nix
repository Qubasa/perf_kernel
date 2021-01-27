  { pkgs ? import <nixpkgs> {}, unstable ? import <nixos-unstable> {} }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      zlib.out
      unstable.rustup
      nasm
      xorriso
      grub2
      entr
      llvmPackages.lld
    ];
    shellHook = ''
      export HISTFILE=${toString ./.history}
      export PATH=$PATH:~/.cargo/bin
      '';
  }

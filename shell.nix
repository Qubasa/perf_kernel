  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      llvmPackages.bintools
      bridge-utils
      tunctl
      zlib.out
      rustup
      xorriso
      grub2
      entr
      llvmPackages.lld
      python3
      python38Packages.pip
      python38Packages.scapy
      python38Packages.ipython
      python38Packages.cryptography
      docker-compose
    ];
    shellHook = ''
      export HISTFILE=${toString ./.history}
      export PATH=$PATH:~/.cargo/bin
      export PATH=$PATH:~/.rustup/toolchains/nightly-2021-01-28-x86_64-unknown-linux-gnu/bin/
      '';
  }

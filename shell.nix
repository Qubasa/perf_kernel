  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      llvmPackages.bintools
      zlib.out
      rustup
      xorriso
      grub2
      entr
      qemu
      llvmPackages.lld
      python3
    ];
    shellHook = ''
      export HISTFILE=${toString ./.history}
      export PATH=$PATH:~/.cargo/bin
      export PATH=$PATH:~/.rustup/toolchains/nightly-2021-01-28-x86_64-unknown-linux-gnu/bin/
      '';
  }

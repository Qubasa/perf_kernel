  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      llvmPackages.bintools
      bridge-utils
      tunctl
      dhcp
      gitAndTools.git-extras
      arp-scan
      zlib.out
      rustup
      xorriso
      grub2
      entr
      llvmPackages.lld
      python38Packages.scapy
      python38Packages.ipython
      dnsmasq
    ];
    shellHook = ''
      export HISTFILE=${toString ./.history}
      export PATH=$PATH:~/.cargo/bin
      export PATH=$PATH:~/.rustup/toolchains/nightly-2021-01-28-x86_64-unknown-linux-gnu/bin/
      '';
  }

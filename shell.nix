  { pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    buildInputs = with pkgs; [
      linux.dev
      linuxPackages.kernel.dev
      linuxPackages.kernel
    ];
    shellHook = "export HISTFILE=${toString ./.history}";
  }

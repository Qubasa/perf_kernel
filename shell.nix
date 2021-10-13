  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:
  pkgs.mkShell rec {
    buildInputs = with pkgs; [
      llvmPackages_latest.llvm
      llvmPackages_latest.bintools
      zlib.out
      rustup
      xorriso
      grub2
      qemu
      entr
      hotspot
      git-extras
      llvmPackages_latest.lld
      python3
    ] ++ (with pkgs.python39Packages; [
      pyelftools
      intervaltree
    ]);
    RUSTC_VERSION = pkgs.lib.readFile ./rust-toolchain;
    # https://github.com/rust-lang/rust-bindgen#environment-variables
    LIBCLANG_PATH= pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
    RUSTFLAGS = (builtins.map (a: ''-L ${a}/lib'') [
    ]);
    BINDGEN_EXTRA_CLANG_ARGS = 
    # Includes with normal include path
    (builtins.map (a: ''-I"${a}/include"'') [
      pkgs.glibc.dev 
    ])
    # Includes with special directory paths
    ++ [
      ''-I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
    ];
    HISTFILE=toString ./.history;
    shellHook = ''
      export PATH=$PATH:~/.cargo/bin
      export PATH=$PATH:~/.rustup/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
      '';
  }

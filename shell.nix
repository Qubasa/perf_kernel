  { sources ? import ./nix/sources.nix
  , pkgs ? import sources.nixpkgs {}
  }:

  let 
  myipxe = pkgs.ipxe.override {
        # pixiecore with the flag --ipxe-ipxe delivers a custom
        # ipxe payload, and the embedded ipxe script sucks. This
        # is my fix. 
        embedScript = pkgs.writeText "ipxe_script" ''
          #!ipxe
          dhcp
          autoboot
          shell
        '';
      };
  in 
  pkgs.mkShell rec {
    buildInputs = with pkgs; [
      zlib.out
      rustup
      xorriso
      pixiecore
      myipxe
      grub2
      qemu
      entr
      git-extras
      python3
    ] ++ (with pkgs.python39Packages; [
      pyelftools
      intervaltree
    ]) ++ (with pkgs.llvmPackages_latest; [
      lld
      bintools
      llvm
    ]);
    IPXE =  myipxe;
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

# This file is pretty general, and you can adapt it in your project replacing
# only `name` and `description` below.

{
  description = "My awesome Rust project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    vmsh-flake = {
      url = "github:mic92/vmsh";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-codium = {
      url = "github:luis-hebendanz/nixos-codium";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-parse-gdt = {
      url = "github:luis-hebendanz/parse-gdt";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-ipxe = {
      url = "github:luis-hebendanz/ipxe?ref=multibootv2";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    glue-gun = {
      url = "github:luis-hebendanz/glue_gun";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self,nix-fenix, nixpkgs, naersk, glue-gun, nix-ipxe, nix-parse-gdt, vmsh-flake, flake-utils, nixos-codium, ... }:
    flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        tmpdir = "/tmp/perfkernel";
        deterministic-git = nixpkgs.outPath + "/pkgs/build-support/fetchgit/deterministic-git";
        fenix = nix-fenix.packages.${system};
       
        target64 = fenix.targets."x86_64-unknown-none".latest.withComponents [
          "rust-std"
        ];
        myrust = with fenix; fenix.combine [
          (latest.withComponents [
            "rust-src"
            "rustc"
            "rustfmt"
            "llvm-tools-preview"
            "cargo"
            "clippy"
          ])
          target64
        ];
        naersk-lib = pkgs.callPackage naersk {
          cargo = myrust;
          rustc = myrust;
        };
        overlays = [ nix-fenix.overlay ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        vmsh = vmsh-flake.packages.${system}.vmsh;
        parse-gdt = nix-parse-gdt.packages.${system}.default;
        glue_gun = glue-gun.packages.${system}.default;

        myipxe = nix-ipxe.packages.${system}.default.override {
          # Script fixes race condition where router dns replies first
          # and pxe boot server second
          embedScript = pkgs.writeText "ipxe_script" ''
            #!ipxe
            dhcp
            autoboot
            shell
          '';
        };
        mycodium = import ./vscode.nix {
          vscode = nixos-codium.packages.${system}.default;
          inherit pkgs;
          vscodeBaseDir = tmpdir + "/codium";
        };

        runtimeDeps = with pkgs; [
          myipxe
          vmsh
          parse-gdt
          mycodium
          radare2
          evcxr # rust repl
          cargo-tarpaulin # for code coverage
          rust-analyzer-nightly
          dhcp
          qemu
          entr # bash file change detector
          netcat-gnu
          git-extras
          python3
          bochs
          llvmPackages_latest.bintools
        ] ++ (with pkgs.python39Packages; [
          pyelftools
          intervaltree
        ]);

        buildDeps = with pkgs; [
          glue_gun
          myrust
          zlib.out
          xorriso
          grub2
        ]  ++ (with pkgs.llvmPackages_latest; [
          lld
          llvm
        ]);
      in
      rec {

        # Needs to be build with: nix build '.?submodules=1'
        # For specific package: nix build '.?submodules=1'#mypackage
        # Issue: https://github.com/NixOS/nix/pull/5284
        packages.default = naersk-lib.buildPackage {
          src = self;
          buildInputs = buildDeps;
          root = ./kernel;
          preBuild = "cd kernel";
          singleStep = true;
        };

        # packages.i686-unknown-none =  naersk-lib.buildPackage {
        #   src = ./.;
        #   buildInputs = buildDeps;
        #   root = ./deps/perf_bootloader;
        #   preBuild = "cd deps/perf_bootloader";

        #   singleStep = true;
        # };

        defaultPackage = packages.default;

        devShells.default = pkgs.mkShell {
          packages = buildDeps ++ runtimeDeps;

          IPXE = myipxe;

          LIBCLANG_PATH = pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];

          BINDGEN_EXTRA_CLANG_ARGS =
            # Includes with normal include path
            (builtins.map (a: ''-I"${a}/include"'') [
              pkgs.glibc.dev
            ])
            # Includes with special directory paths
            ++ [
              ''-I"${pkgs.llvmPackages_latest.libclang.lib}/lib/clang/${pkgs.llvmPackages_latest.libclang.version}/include"''
            ];

          shellHook = ''
            TMP=${tmpdir}
            mkdir -p $TMP
            export HISTFILE=$TMP/.history
            export CARGO_HOME=$TMP/cargo
            export PATH=$PATH:$TMP/cargo/bin
          '';
        };

      });
}

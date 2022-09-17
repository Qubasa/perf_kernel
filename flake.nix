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
    nix-luispkgs.url = "github:Luis-Hebendanz/nixpkgs/fix_buildRustPackage";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nci = {
      url = "github:yusdacra/nix-cargo-integration";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs = { self, nixpkgs, naersk, nci, rust-overlay,  nix-luispkgs, glue-gun, nix-ipxe, nix-parse-gdt, vmsh-flake, flake-utils, nixos-codium, ... }:
    flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        naersk-lib = pkgs.callPackage naersk { };
        overlays = [ (import rust-overlay) ];
        luispkgs = import  nix-luispkgs {
          inherit system overlays;
        };
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        myrust = pkgs.rust-bin.nightly."2021-10-26".default.override {
          extensions = [ "rustfmt" "llvm-tools-preview" "rust-src" ];
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
          vscodeBaseDir = "/tmp/nixos-codium-perfkernel";
        };

        buildDeps = with pkgs; [
          myipxe
          vmsh
          glue_gun
          parse-gdt
          mycodium
          myrust
          evcxr # rust repl
          cargo-tarpaulin # for code coverage
          rust-analyzer
          zlib.out
          xorriso
          dhcp
          grub2
          qemu
          entr # bash file change detector
          #glibc.dev # Creates problems with tracy
          netcat-gnu
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
      in
      rec {
        packages.default = luispkgs.rustPlatform.buildRustPackage {
          pname = "perfkernel";
          version = "0.0.1";

          src = ./.;
          #sourceRoot = ./.;

          cargoLock = {
            lockFile = ./kernel/Cargo.lock;
          };

          #cargoVendorDir = ./kernel;

          meta = with pkgs.lib; {
            description = "x64 rust multicore kernel optimized for extreme performance at any cost.";
            homepage = "https://github.com/Luis-Hebendanz/perf_kernel";
            license = licenses.unlicense;
            maintainers = [ maintainers.luis ];
          };
        };
        defaultPackage = packages.default;

        devShells.default = pkgs.mkShell {
          buildInputs = buildDeps;

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

          HISTFILE = toString ./.history;
        };

      });
}

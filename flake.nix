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
    nix-fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self, nixpkgs,  nix-fenix, glue-gun, crane,
    nix-ipxe, nix-parse-gdt, vmsh-flake, flake-utils,
    nixos-codium, ... } :
    flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        fenix = nix-fenix.packages.${system};
        overlays = [ nix-fenix.overlay ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = fenix.toolchainOf {
          channel = "nightly";
          date = "2021-10-26";
          sha256 = "sha256-1hLbypXA+nuH7o3AHCokzSBZAvQxvef4x9+XxO3aBao=";
        };
        myrust = toolchain.withComponents [
          "rustc"
          "rustfmt"
          "llvm-tools-preview"
          "rust-src"
          "cargo"
          "clippy"
        ];
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
          cargo-tarpaulin
          myipxe
          vmsh
          glue_gun
          parse-gdt
          mycodium
          myrust
          evcxr # rust repl
          rust-analyzer-nightly
          zlib.out
          xorriso
          dhcp
          grub2
          qemu
          entr # bash file change detector
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
        packages.default = crane.lib.${system}.buildPackage {
          src = ./.;
          
        };

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

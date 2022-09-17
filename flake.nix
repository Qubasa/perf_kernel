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
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    dream2nix.url = "github:nix-community/dream2nix";
  };

  outputs = {
    self, nixpkgs,  fenix, dream2nix, glue-gun,
    nix-ipxe, nix-parse-gdt, vmsh-flake, flake-utils,
    nixos-codium, ... } :
      let
        system = "x86_64-linux";
        overlays = [ (import fenix.overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = fenix.toolchainOf {
          channel = "nightly";
          date = "2021-10-26";
        };
        myrust = toolchain.withComponents [
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
      (dream2nix.lib.makeFlakeOutputs {
        systems = [ system ];
        config.projectRoot = ./kernel;
        config.disableIfdWarning = true;
        source = ./.;
        settings = [
          {
            builder = "crane";
            translator = "cargo-toml";
          }
        ];
        packageOverrides = {
          # override all packages and set a toolchain
          "^.*" = {
            set-toolchain.overrideRustToolchain = old: { cargo = toolchain; };
            check-toolchain-version.overrideAttrs = old: {
              buildPhase = ''
                currentCargoVersion="$(cargo --version)"
                customCargoVersion="$(${toolchain}/bin/cargo --version)"
                if [[ "$currentCargoVersion" != "$customCargoVersion" ]]; then
                  echo "cargo version is $currentCargoVersion but it needs to be $customCargoVersion"
                  exit 1
                fi
                ${old.buildPhase or ""}
              '';
            };
          };
        };
      }) // {
      checks.x86_64-linux.perf_kernel = self.packages.x86_64-linux.perf_kernel;
    };
}

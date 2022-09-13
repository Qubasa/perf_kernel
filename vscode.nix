{ pgks, vscode, ... }:


vscode.override {
  vscodeBaseDir = toString ./.vscode;
  nixExtensions = pkgs.vscode-utils.extensionsFromVscodeMarketplace [
    {
      name = "language-x86-64-assembly";
      publisher = "13xforever";
      version = "3.0.0";
      sha256 = "sha256-wIsY6Fuhs676EH8rSz4fTHemVhOe5Se9SY3Q9iAqr1M=";
    }
    {
      name = "vscode-coverage-gutters";
      publisher = "ryanluker";
      version = "2.8.2";
      sha256 = "sha256-gMzFI0Z9b7I7MH9v/UC7dXCqllmXcqHVJU7xMozmMJc=";
    }
    {
      name = "llvm";
      publisher = "rreverser";
      version = "0.1.1";
      sha256 = "sha256-MPY854kj34ijQqAZQCSvdszanBPYzxx1D7m+3b+DqGQ=";
    }
  ] ++ (with pkgs.vscode-extensions;  [
    yzhang.markdown-all-in-one
    timonwong.shellcheck
    tamasfe.even-better-toml
    serayuzgur.crates
    jnoortheen.nix-ide
    matklad.rust-analyzer
    vadimcn.vscode-lldb
    #ms-python.python # Broken on nixos unstable
  ]);
  settings = {
    window.menuBarVisibility = "toggle";
    window.zoomLevel = 0;
    editor.fontSize = 16;
    terminal.integrated.fontSize = 16;
    lldb.displayFormat = "hex";
    breadcrumbs.enabled = false;
    files.associations."*.s" = "asm-intel-x86-generic";
    rust-analyzer.inlayHints.parameterHints = false;
    workbench.colorCustomizations = {
      statusBar.background = "#1A1A1A";
      statusBar.noFolderBackground = "#212121";
      statusBar.debuggingBackground = "#263238";
    };
  };

  keybindings = [
    {
      key = "f6";
      command = "workbench.action.tasks.runTask";
      args = "rust: cargo run";
    }
    {
      key = "f4";
      command = "workbench.action.tasks.runTask";
      args = "Debug kernel";
    }
  ];
}

#Use vscodeWithConfiguration and vscodeExts2nix to create a vscode executable. When the executable exits, it updates the mutable extension file, which is imported when evaluated by Nix later.
{ lib
, buildEnv
, writeShellScriptBin
, extensionsFromVscodeMarketplace
, vscodeDefault
, writeScript
, jq
}:
##User input
{ vscode                           ? vscodeDefault
, nixExtensions                    ? []
, vscodeBaseDir                    ? ".vscode"
, settings                         ? {}
, launch                           ? {}
, keybindings                      ? {}
}:
let
  user-data-dir = vscodeBaseDir + "/user-data-dir";
  vscodeExtsFolderName = vscodeBaseDir + "/vscode-extensions";


  vscodeWithConfiguration = import ./vscodeWithConfiguration.nix {
    inherit lib writeShellScriptBin extensionsFromVscodeMarketplace writeScript jq;
    vscodeDefault = vscode;
  }{ inherit nixExtensions vscodeExtsFolderName user-data-dir; };

  updateSettings = import ./updateSettings.nix { inherit lib writeShellScriptBin jq; };

  updateSettingsCmd = updateSettings {
    settings = {
        "extensions.autoCheckUpdates" = false;
        "extensions.autoUpdate" = false;
        "update.mode" = "none";
    } // settings;
    vscodeBaseDir = user-data-dir;
    vscodeSettingsFile = "settings.json";
  };

  updateLaunchCmd = updateSettings {
    vscodeBaseDir = user-data-dir;
    settings = launch;
    vscodeSettingsFile =  "launch.json";
  };

  updateKeybindingsCmd = updateSettings {
    settings = keybindings;
    vscodeSettingsFile = "keybindings.json";
    vscodeBaseDir = user-data-dir;
  };

  code = writeShellScriptBin "code" ''
    ${updateSettingsCmd}/bin/vscodeNixUpdate-settings
    ${updateLaunchCmd}/bin/vscodeNixUpdate-launch
    ${updateKeybindingsCmd}/bin/vscodeNixUpdate-keybindings
    ${vscodeWithConfiguration}/bin/${vscode.executableName} --wait "$@"
  '';
in
buildEnv {
  name = "vscodeEnv";
  paths = [ code updateSettingsCmd updateLaunchCmd updateKeybindingsCmd ];
}

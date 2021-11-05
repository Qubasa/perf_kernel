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
, vscodeExtsFolderName             ? ".vscode-exts"
# will add to the command updateSettings (which will run on executing vscode) settings to override in settings.json file
, settings                         ? {}
, createSettingsIfDoesNotExists    ? true
, launch                           ? {}
, createLaunchIfDoesNotExists      ? true
# will add to the command updateKeybindings(which will run on executing vscode) keybindings to override in keybinding.json file
, keybindings                      ? {}
, createKeybindingsIfDoesNotExists ? true
, user-data-dir ? ''"''${TMP}/''${name}"/vscode-data-dir''

}:
let
  vscodeWithConfiguration = import ./vscodeWithConfiguration.nix {
    inherit lib writeShellScriptBin extensionsFromVscodeMarketplace writeScript;
    vscodeDefault = vscode;
  }
  {
    inherit nixExtensions vscodeExtsFolderName user-data-dir;
  };

  updateSettings = import ./updateSettings.nix { inherit lib writeShellScriptBin jq; };
  userSettingsFolder = "${ user-data-dir }/User";

  updateSettingsCmd = updateSettings {
    settings = {
        "extensions.autoCheckUpdates" = false;
        "extensions.autoUpdate" = false;
        "update.mode" = "none";
    } // settings;
    inherit userSettingsFolder;
    createIfDoesNotExists = createSettingsIfDoesNotExists;
    symlinkFromUserSetting = (user-data-dir != "");
  };

  updateLaunchCmd = updateSettings {
    settings = launch;
    createIfDoesNotExists = createLaunchIfDoesNotExists;
    vscodeSettingsFile = ".vscode/launch.json";
  };

  updateKeybindingsCmd = updateSettings {
    settings = keybindings;
    createIfDoesNotExists = createKeybindingsIfDoesNotExists;
    vscodeSettingsFile = ".vscode/keybindings.json";
    inherit userSettingsFolder;
    symlinkFromUserSetting = (user-data-dir != "");
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

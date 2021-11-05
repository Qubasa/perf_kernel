# Updates the vscode setting file base on a nix expression
# should run from the workspace root.
{ writeShellScriptBin
, lib
, jq
}:
##User Input
{ settings      ? {}
# if marked as true will create an empty json file if does not exist
, vscodeSettingsFile
, vscodeBaseDir
}:
let
  vscodeUserDir = vscodeBaseDir + "/User";
  vscodeSettingsFilePath = vscodeUserDir + "/" + vscodeSettingsFile;
in

  writeShellScriptBin ''vscodeNixUpdate-${lib.removeSuffix ".json" (vscodeSettingsFile)}''
  ''
    mkdir -p ${vscodeUserDir}
    echo '${builtins.toJSON settings}' | ${jq}/bin/jq > ${vscodeSettingsFilePath}
  ''

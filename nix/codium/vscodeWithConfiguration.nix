# wrapper over vscode to control extensions per project (extensions folder will be created in execution path)
{ lib
, writeShellScriptBin
, writeScript
, jq
, extensionsFromVscodeMarketplace
, vscodeDefault
}:
## User input
{ vscode ? vscodeDefault
# extensions to be symlinked into the project's extensions folder
, nixExtensions        ? []
, vscodeExtsFolderName
, user-data-dir
}:
let
  nixExtsDrvs = nixExtensions;
  vscodeExtName = import ./vscodeExtName.nix { inherit lib; inherit writeScript; inherit jq; };

  #removed not defined extensions
  rmExtensions =  lib.optionalString (nixExtensions != null && lib.length nixExtensions != 0) ''
    find ${vscodeExtsFolderName} -mindepth 1 -maxdepth 1 ${
        lib.concatMapStringsSep " " (e : "! -iname $(${vscodeExtName} ${e})") nixExtensions
      } -exec rm -rf {} \;
  '' + lib.optionalString (nixExtensions == null || lib.length nixExtensions == 0) ''
    if [ -d "${vscodeExtsFolderName}" ]; then
      rm -r "${vscodeExtsFolderName}"
    fi
  '';

  #copy mutable extension out of the nix store
  cpExtensions = ''
    ${lib.concatMapStringsSep "\n" (e : "ln -sfn ${e}/share/vscode/extensions/* ${vscodeExtsFolderName}/") nixExtsDrvs}
  '';
in
  {
  executableName = vscode.executableName;
  outPath = writeShellScriptBin "${vscode.executableName}" ''
    if ! [[ "$@" =~ "--list-extension" ]]; then
      mkdir -p "${vscodeExtsFolderName}"
      ${rmExtensions}
      ${cpExtensions}
    fi
    ${vscode}/bin/${vscode.executableName} --extensions-dir "${vscodeExtsFolderName}" ${
      lib.optionalString (user-data-dir != "") "--user-data-dir ${user-data-dir}"
      } "$@" &
  '';
  }

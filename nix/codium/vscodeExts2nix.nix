# based on the passed vscode will stdout a nix expression with the installed vscode extensions
{ lib
, vscodeDefault
, writeShellScriptBin
, writeScript
}:

##User input
{ vscode             ? vscodeDefault
, extensionsToIgnore ? []
# will use those extensions to get sha256 if still exists when executed.
, extensions         ? []
}:
let
  mktplcExtRefToFetchArgs = import ./mktplcExtRefToFetchArgs.nix;
  vscodeExtName = import ./vscodeExtName.nix { inherit lib; inherit writeScript; };

in
writeShellScriptBin "vscodeExts2nix" ''
  echo '['

  for line in $(${vscode.outPath}/bin/${vscode.executableName} --list-extensions --show-versions \
    ${lib.optionalString (extensionsToIgnore != []) ''
      | grep -v -i '^\(${lib.concatMapStringsSep "\\|" (e : "$(${vscodeExtName e})") extensionsToIgnore}\)'
    ''}
  ) ; do
    [[ $line =~ ([^.]*)\.([^@]*)@(.*) ]]
    name=''${BASH_REMATCH[2]}
    publisher=''${BASH_REMATCH[1]}
    version=''${BASH_REMATCH[3]}

    extensions="${lib.concatMapStringsSep "." (e : "${vscodeExtName e}@${e.src.outputHash}") extensions}"
    reCurrentExt=$publisher$name"@([^.]*)"
    if [[ $extensions =~ $reCurrentExt ]]; then
      sha256=''${BASH_REMATCH[1]}
    else
      sha256=$(
        nix-prefetch-url "${(mktplcExtRefToFetchArgs {publisher = ''"$publisher"''; name = ''"$name"''; version = ''"$version"'';}).url}" 2> /dev/null
      )
    fi

    echo "{ name = \"''${name}\"; publisher = \"''${publisher}\"; version = \"''${version}\"; sha256 = \"''${sha256}\";  }"
  done


  echo ']'
''

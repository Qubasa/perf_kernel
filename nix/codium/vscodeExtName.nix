{
    lib,
    jq,
    writeScript
}:

writeScript "vscodeExtName" ''
NAME=$(cat $1/share/vscode/extensions/*/package.json | ${jq}/bin/jq -r '.name' | tr '[:upper:]' '[:lower:]')
PUB=$(cat $1/share/vscode/extensions/*/package.json | ${jq}/bin/jq -r '.publisher' | tr '[:upper:]' '[:lower:]')

echo "''${PUB}.$NAME"
''
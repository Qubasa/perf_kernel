#!/usr/bin/env bash
set -euxo pipefail

nm "$1" | grep " T " | awk '{ print $1" "$3 }' > "$1".bochs
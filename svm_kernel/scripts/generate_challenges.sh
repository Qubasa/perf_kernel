#!/usr/bin/env bash

set -e

if ! command -v cargo &> /dev/null
then
   echo "cargo command could not be found"
   exit 1
fi

# TODO: Put here the IP of the kernel for every team
TEAMS=("192.168.178.54" "192.168.177.54")
# Put here the gateway for all teams
GATEWAY="192.168.178.1"

BASE="scripts/"
if [ "$(basename "$PWD")" == "scripts" ]; then
   BASE=""
   cd ..
elif [ "$(basename "$PWD")" != "svm_kernel" ]; then
   echo "ERROR: Wrong base directory: $PWD"
   exit 1
fi

# If scripts gets aborted with ctrl+c
function cleanup() {
   sed -i 's/\["build", "--release"\]/\["build"\]/g' Cargo.toml
}
trap cleanup INT
trap cleanup 0

# Set Cargo.toml to release build
sed -i 's/\["build"\]/\["build", "--release"\]/g' Cargo.toml

COUNTER=0

for team in "${TEAMS[@]}"; do

   COUNTER=$((COUNTER+1))
   echo "====== Generating challenge for $team ======"
   ./${BASE}set_static_ip.sh "$team" "$GATEWAY"
   cargo bootimage --grub
   mkdir -p "out/$team"
   cp target/x86_64-os/release/bootimage-svm_kernel.iso "out/$team/kernel_mania.iso"
   cp ./${BASE}tap_interface.sh "out/$team"
   cat >"out/$team/run.sh" <<EOF
#!/bin/sh

ip link show dev kmania_br0 &> /dev/null
if [ 1 -eq \$? ]; then
   echo "Please execute tap_interface.sh first"
   exit 1
fi

function cleanup() {
   exit 0
}
trap cleanup INT

while [ "1" == "1" ]; do
   qemu-system-x86_64 -cpu EPYC-v1 -smp cores=1 -cdrom ./kernel_mania.iso -serial stdio -display none -device isa-debug-exit,iobase=0xf4,iosize=0x04 -m 1G -netdev tap,id=mynet0,ifname=kmania_tap0,script=no,downscript=no -device rtl8139,netdev=mynet0,mac=52:55:00:d1:55:$(printf "%02x" $COUNTER)
   echo "===== Restarting machine... ====="
done
EOF
    chmod +x "out/$team/run.sh"
done # end for loop


sed -i 's/\["build", "--release"\]/\["build"\]/g' Cargo.toml
echo "Challenges saved to out/"

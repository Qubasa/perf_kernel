#!/usr/bin/env bash

set -e

# Put here the IP of the kernel for every team
TEAMS=("192.168.178.54" "192.168.177.54" "192.168.176.54")

# If scripts gets aborted with ctrl+c
function ctrl_c() {
    sed -i 's/\["build", "--release"\]/\["build"\]/g' Cargo.toml
}
trap ctrl_c INT

# Set Cargo.toml to release build
sed -i 's/\["build"\]/\["build", "--release"\]/g' Cargo.toml

COUNTER=0

for team in "${TEAMS[@]}"; do

    COUNTER+=1
    echo "Generating challenge for $team"
    ./set_static_ip.sh "$team"
    cargo bootimage --grub
    mkdir -p "out/$team"
    cp target/x86_64-os/release/bootimage-svm_kernel.iso "out/$team/kernel_mania.iso"
    cat >"out/$team/run.sh" <<EOF
#!/bin/sh

# Check if script is executed as root
if [[ \$EUID -ne 0 ]]; then
   echo "[-] This script must be run as root" 1>&2
   exit 1
fi

qemu-system-x86_64 -cpu EPYC-v1 -smp cores=1 -cdrom ./kernel_mania.iso -serial stdio -display none -device isa-debug-exit,iobase=0xf4,iosize=0x04 -m 1G -netdev tap,id=mynet0,ifname=tap0,script=no,downscript=no -device rtl8139,netdev=mynet0,mac=52:55:00:d1:55:$COUNTER
EOF
    chmod +x "out/$team/run.sh"
done # end for loop


sed -i 's/\["build", "--release"\]/\["build"\]/g' Cargo.toml
echo "[+] Done!"

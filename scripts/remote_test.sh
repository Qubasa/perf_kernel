#!/usr/bin/env bash

TARGET=${1:-"sandro@nardole.r"}

ssh "$TARGET" "
mkdir -p /tmp/qubasa
"

rsync -Pav kernel/target/x86_64-unknown-none/debug/perf_kernel.iso "$TARGET":/tmp/qubasa
ssh $TARGET "
cd /tmp/qubasa
nix shell nixpkgs#qemu_kvm -c qemu-kvm -monitor tcp:localhost:8124,server,nowait -no-reboot -cpu host -smp cores=\$(nproc) -cdrom perf_kernel.iso -display none -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04 -m 4G -name perf_kernel,process=perf_kernel
"

ssh $TARGET "
pkill perf_kernel
"


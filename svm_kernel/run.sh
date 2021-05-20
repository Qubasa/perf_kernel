#!/bin/sh

qemu-system-x86_64 -cpu EPYC-v1 -smp cores=1 -cdrom /kernel_mania/kernel_mania.iso -serial stdio -display none -device isa-debug-exit,iobase=0xf4,iosize=0x04 -m 1G -netdev tap,id=mynet0,ifname=tap0,script=no,downscript=no -device rtl8139,netdev=mynet0,mac=52:55:00:d1:55:01



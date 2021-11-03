#!/usr/bin/env bash

gdb -ex "target remote: 1234" -ex "symbol-file ~/Projects/svm_kernel/kernel/target/x86_64-os/debug/perf_kernel"
 # -ex "symbol-file ~/Projects/svm_kernel/kernel/target/x86_64-os/debug/isofiles-perf_kernel/boot/kernel.elf"

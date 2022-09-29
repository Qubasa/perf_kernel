#!/usr/bin/env bash

gdb -ex "target remote: 1234" -ex "symbol-file kernel/target/x86_64-unknown-none/debug/perf_kernel"
 # -ex "symbol-file kernel/target/x86_64-unknown-none/debug/isofiles-perf_kernel/boot/kernel.elf"

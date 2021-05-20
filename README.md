## ==== WORK IN PROGRESS ====
Working on multicore support

## Description
x86_64 HPC AMD kernel written in Rust.
Optimized for hypervisor usage.


## Setup & Debug Build
Clone the repo with submodules:
```
$ git clone --recursive <url>
```

Pinned rustc version is found in [rust-toolchain](svm_kernel/rust-toolchain)

Install the dependencies listed in `shell.nix` or execute
`nix-shell shell.nix` if on NixOS.

Install cargo dependencies:
```
$ cargo install -p svm_kernel/bootimage
$ rustup component add llvm-tools-preview rustc-src
```

Setup network:
```bash
$ sudo ./svm_kernel/tap_interface.sh
```
Set static ip of kernel:
```
$ ./svm_kernel/set_static_ip.sh <kernel_ip> <router_ip>
```


Run in qemu with:
```bash
$ cargo run
```
Close the instance with CTRL+A,X
or CTRL+C

Build on filechange:
```bash
$ cd svm_kernel
$ cargo watch
```

Run exploit with:
```
$ sudo ./checker/src/icmp.py <kernel_ip>
```

Checker works now, you need to edit [checker.py](https://github.com/enowars/enowars5-service-kernel_mania/blob/enowars/checker/src/checker.py#L39) and change `test_ip` to `None` in production or your local kernel ip for testing.

To test the exploit execute: `checker/src/icmp.py <ip>`

## Release build:
Execute:
```bash
$ cargo run --release
```
The resulting file lies in: `target/x86_64-os/release/bootimage-svm_kernel.bin`
Flash it with:
```bash
$ dd bs=5M if=target/x86_64-os/release/bootimage-svm_kernel.iso of=/dev/MYDEVICE
```

OR
Edit the file `Cargo.toml` and change `build-command` to `["build", "--release"]`
Then execute `cargo bootimage --grub`

## Generate & view assembly
```
$ cargo asm
```

You can find the asm file in `target/x86_64-os/release/deps/svm_kernel-*.s`


## Debug with gdb
```bash
$ qemu-kvm -cpu host -smp cores=4 -cdrom target/x86_64-os/debug/bootimage-svm_kernel.iso -serial stdio -display none -device isa-debug-exit,iobase=0xf4,iosize=0x04 -m 2G
```
In another shell execute:
```bash
$ gdb target/x86_64-os/debug/isofiles/boot/kernel.elf -ex "target remote:1234"
```
You have to use `hb` instead of `b` in gdb when using qemu-kvm. If not the breakpoints get ignored.

To get debug symbols for the kernel and not for the bootloader execute:
```
(gdb) symbol-file target/x86_64-os/debug/svm_kernel
```

If you want to debug other cores you have to use qemu in emulation mode and not in kvm mode!
If qemu is in emulation mode gdb sees other cores as threads thus settings breakpoints has to be done
as follows:
List all cores and its IDs:
```
(gdb) thread
```
Set breakpoint
```
(gdb) break <location> thread <thread-id>
```


## Debug with radare2
```bash
$ r2 target/x86_64-os/debug/isofiles/boot/kernel.elf # Debug bootloader
```
```bash
$ r2 target/x86_64-os/debug/svm_kernel # Debug kernel
```

Look into [svm_kernel/external/bootloader/linker.ld](svm_kernel/external/bootloader/linker.ld) to find the offset where the kernel gets mapped to.

## Run tests
To execute tests run:
```
$ cargo test
```
Run specific test:
```
$ cargo test --test heap_allocator
```

## Developed on a
* AMD Ryzen 5 3500U
* EPYC-v1
* AMD Family 17h Model 18h

## Resources
* https://os.phil-opp.com/
* https://www.amd.com/system/files/TechDocs/24593.pdf
* https://github.com/gamozolabs/chocolate_milk/
* https://uefi.org/sites/default/files/resources/ACPI_6_3_final_Jan30.pdf
* [Use 1Gib pages sparringly](https://forum.osdev.org/viewtopic.php?f=1&t=32699)
* [Don't touch MTRRs](https://forum.osdev.org/viewtopic.php?t=29034&p=246311)
* https://virtio-fs.gitlab.io/index.html#overview
* https://gitlab.redox-os.org/redox-os/tfs
* http://9p.cat-v.org/
* https://www.linux-kvm.org/page/Tuning_Kernel




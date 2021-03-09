## ==== WORK IN PROGRESS ====
Working grub bootloader, currently implementing mode switch to long mode

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
$ rustup component add llvm-tools-preview
```

Run in qemu with:
```
$ cargo run
```
Close the instance with CTRL+A,X
or CTRL+C

Build on filechange:
```
$ cd svm_kernel
$ cargo watch
```

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
$ qemu-kvm -cpu qemu64,+svm,vendor=AuthenticAMD -drive format=raw,file=target/x86_64-os/debug/bootimage-svm_kernel.bin -nographic -s -S
```
In another shell execute:
```bash
$ gdb target/x86_64-os/debug/svm_kernel.d -ex "target remote:1234"
```
You have to use `hb` instead of `b` in gdb when using qemu-kvm. If not the breakpoints get ignored.
Note: `svm_kernel.d` are the extracted symbols from the svm_kernel binary.

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
```
$ r2 -B [TODO] target/x86_64-os/debug/svm_kernel
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



## Resources
* https://os.phil-opp.com/
* https://www.amd.com/system/files/TechDocs/24593.pdf
* https://github.com/gamozolabs/chocolate_milk/
* https://uefi.org/sites/default/files/resources/ACPI_6_3_final_Jan30.pdf
* [Use 1Gib pages sparringly](https://forum.osdev.org/viewtopic.php?f=1&t=32699)
* [Don't touch MTRRs](https://forum.osdev.org/viewtopic.php?t=29034&p=246311)




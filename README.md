## Description
==== WORK IN PROGRESS ====

Trying to get multicore to work.

Goal:
x86_64 AMD kernel optimized for extreme performance at the cost of ditching all security measures.
In the future this should become a hypervisor.

## Setup & Debug Build
Clone the repo with submodules:
```bash
$ git clone --recursive <url>
$ git checkout enowars
```


Install the [nix package manager](https://nixos.org/download.html).  
The installation script requires that you have `sudo` access to `root`.
```bash
$ curl -L https://nixos.org/nix/install | sh
```

To download all required pinned dependencies just execute:
```bash
$ cd svm_kernel
$ nix-shell shell.nix
```

Then install cargo dependencies:
```bash
$ cd svm_kernel
$ cargo install --path bootimage
$ rustup component add llvm-tools-preview rust-src
```

Now compile & run the kernel in qemu with:
```bash
$ cargo run
```
Close the instance with CTRL+C

To build on filechange:
```bash
$ cargo install cargo-watch
$ cargo watch
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




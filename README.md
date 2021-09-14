## Description
A unicore kernel with a custom icmp protocol, with some vulnerabilities


## Setup & Debug Build
Clone the repo with submodules:
```
$ git clone --recursive <url>
$ git checkout enowars
```

Install the dependencies listed in [shell.nix](shell.nix), make sure to also add the PATH variables to your environment.
Or execute `nix-shell shell.nix` if on NixOS or by installing the [nix package manager](https://nixos.org/download.html) (highly recommended)

Install cargo dependencies:
```
$ cargo install --path bootimage
$ rustup component add llvm-tools-preview rust-src
```

Run in qemu with:
```bash
$ cargo run
```
Close the instance with CTRL+C

Build on filechange:
```bash
$ cd svm_kernel
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




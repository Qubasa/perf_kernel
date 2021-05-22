## Description
A unicore kernel with an custom icmp protocol, with some vulnerabilities

## Setup & Debug Build
Clone the repo with submodules:
```
$ git clone --recursive <url>
```

Install the dependencies listed in [shell.nix](shell.nix), make sure to also add the PATH variables to your environment.
Or execute `nix-shell shell.nix` if on NixOS.

Install cargo dependencies:
```
$ cargo install -p svm_kernel/bootimage
$ rustup component add llvm-tools-preview rustc-src
```

Then you need to setup a tap interface owned by your build user. For this
execute the following script:
```bash
$ sudo ./svm_kernel/scripts/tap_interface.sh <username>
```

Set static ip of kernel:
```
$ ./svm_kernel/scripts/set_static_ip.sh <kernel_ip> <router_ip>
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

Run exploit with:
```
$ sudo ./checker/src/icmp.py <kernel_ip>
```

Checker works now, you need to edit [checker.py](https://github.com/enowars/enowars5-service-kernel_mania/blob/enowars/checker/src/checker.py#L39) and change `test_ip` to `None` in production or your local kernel ip for testing.

Execute checker normally with:
```bash
$ cd checker
$ docker-compose up --build
```

Then visit http://localhost:8000/


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




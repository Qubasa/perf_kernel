# Perf Kernel

This is a research multicore x86_64 kernel that supports AVX and SSE with a focus on introspection and debugging capabilities.  

## Setup & Debug Build
Clone the repo with submodules:
```bash
$ git clone --recursive <url>
```

Install the [nix package manager](https://nixos.org/download.html).  
The installation script requires that you have `sudo` access to `root`.
```bash
$ curl -L https://nixos.org/nix/install | sh
```
You may ask yourself why I use this weird package manager. The answer is simple: A completely reproducable and pinned development environment that works across every Linux distribution the same. Also through nix installed packages are contained and have no side effects on your system. 


To download all required pinned dependencies just execute:
```bash
$ cd <project_root>
$ nix-shell shell.nix
```

Then install some cargo dependencies:
```bash
$ cd <project_root>
$ cargo install --path tools/glue_gun
$ rustup component add llvm-tools-preview rust-src
```

Now compile & run the kernel in qemu with:
```bash
$ cd <project_root>/kernel
$ cargo run
```

## Integrated code editor
This projects ships with a customized & pinned vscodium (vscode without telemetry) with all the necessary addons. Included features are:
- in editor kernel debugging with source code breakpoints and variables window
- rust analyzer completion support of kernel code
- clippy linting
- rust optimized dark theme 


Open kernel source
```bash
# Open kernel source code
$ code kernel/kernel.code-workspace

# Open bootloader source code
$ code crates/bootloader/bootloader.code-workspace
```

### Keyboard shortcuts
- `F4` builds and runs the kernel in qemu awaiting a debugger
- `F5` attaches debugger to running kernel
- `F6` builds and runs the kernel normally

## View assembly with radare2
```bash
$ cd <project_root>/kernel
$ r2 target/x86_64-os/debug/isofiles/boot/kernel.elf # View bootloader asm
```
```bash
$ cd <project_root>/kernel
$ r2 target/x86_64-os/debug/perf_kernel # View kernel asm
```

Look into [crates/bootloader/linker.ld](crates/bootloader/linker.ld) to find the offset where the kernel gets mapped to.

## Debug with gdb

To run the kernel in debugger await mode execute:
```bash
$ cargo run -- -d
```

Debugging the *bootloader* with gdb
```bash
$ cd <project_root>/perf_kernel
$ gdb -ex "target remote: 1234" -ex "symbol-file target/x86_64-os/debug/isofiles/boot/kernel.elf"
```

Debugging the *kernel* with gdb
```bash
$ cd <project_root>/perf_kernel
$ gdb -ex "target remote: 1234" -ex "symbol-file target/x86_64-os/debug/perf_kernel"
```

### Debugging a different cpu core 
In gdb cpu cores get handled like threads. So to display all cpu cores execute: `info threads`
To set a breakpoint on a different core execute: `hb <address> thread <cpu_num>`

### Important
If you use qemu with kvm you have to use [hardware breakpoints](https://en.wikipedia.org/wiki/Breakpoint#Implementations). Those are set with `hb <address>`

In qemu emulation mode just use the normal breakpoints set with `b <address>`


## Debug with qemu monitor
Connect to [qemu monitor](https://qemu.readthedocs.io/en/latest/system/monitor.html) with
```
$ nc 127.0.0.1 8124
(qemu) help
```

To switch to a different cpu core, execute:
```
(qemu) cpu <core_num>-1
```

## Linker map
The linker generates a linker map where all ELF objects are listed with their respective addresses.
You can find the file under `<project_root>/perf_kernel/external/bootloader/target/linker.map`.


## Debugging MMU with vmsh
[vmsh](https://github.com/Luis-Hebendanz/vmsh/tree/kernel_inspector) is a tool that spawns a thread in a qemu process to extract the kvm filedescriptor. This enables us to read VM guest memory from the host. The [restart.sh](https://github.com/Luis-Hebendanz/perf_kernel/blob/master/perf_kernel/restart.sh) does all of this automatically and then writes the MMU state as text into `target/dump.analysis`   

Excerpt:
```
Virt Addr         Phys Addr       Size Perms Cache  NX
0x0            -> UNMAPPED        4Kb 
0x1000         -> 0x1000          4Kb  R       PCD NX 
...
0x3000         -> 0x3000          4Kb  R       PCD NX 
0x4000         -> 0x4000          4Kb  W              
0x5000         -> 0x5000          4Kb  R       PCD NX 
...
0xb7000        -> 0xb7000         4Kb  R       PCD NX 
0xb8000        -> 0xb8000         4Kb  W       PCD NX 
0xb9000        -> 0xb9000         4Kb  R       PCD NX 
...
0xff000        -> 0xff000         4Kb  R       PCD NX 
0x100000       -> 0x100000        4Kb  R              
...
```

## ISO file
To create a new ISO file, `cargo run` needs to be executed. `cargo build` does not suffice, it only generates a new kernel executable file but not a new ISO file. The path to the ISO is `target/x86_64-os/debug/bootimage-perf_kernel.iso` or if build in release mode `target/x86_64-os/release/bootimage-perf_kernel.iso`. To create a bootable USB stick just flash the image onto the USB device with:
```bash
$ dd bs=5M if=target/x86_64-os/release/bootimage-perf_kernel.iso of=/dev/<YourUSB> status=progress
```

## PXE boot
Previously I tried to use `pixiecore` to setup PXE however there are a couple of incompatibilities because it always uses it's own IPXE build integrated into the tool.
But IPXE does not currently support Multibootv2 booting, that's why shell.nix builds a custom version of IPXE that can be found under `$IPXE/undionly.kpxe`


## LLVM assembly
If you are interested in the LLVM assembly of your kernel then execute `cargo asm` this generates the LLVM asm in release mode under: `target/x86_64-os/release/deps/perf_kernel-*.s`

## Build system
The build system is highly custom but well integrated into cargo. The [glue_gun](tools/glue_gun/README.md) tool goes into more detail.

Important configuration files for the build system are:
* [.cargo/config.toml](kernel/.cargo/config.toml)
* [Cargo.toml](kernel/Cargo.toml)
* [x86_64-os.json](kernel/x86_64-os.json)
* [i686-uknown-linux-gnu.json](crates/bootloader/i686-unknown-linux-gnu.json)
* [linker.ld](crates/bootloader/linker.ld)
* [build.rs](crates/bootloader/build.rs)
* [rust-toolchain](rust-toolchain)

## Run tests
To execute tests run:
```
$ cd <project_root>/perf_kernel
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





## Build chain 

The file `bootloader/.cargo/config` defines an llvm target file called `i686-unknown-linux-gnu.json`.
This file defines the architecture, sets compile flags and tells llvm to use the linker script `linker.ld`.

The linker script tells the linker at which offsets the sections should be mapped to.
Read more about linker scripts
[here](https://www.sourceware.org/binutils/docs/ld/Scripts.html) 

Another important role plays the file `build.rs`.
Placing a file named `build.rs` in the root of a package will cause
Cargo to compile that script and execute it just before building the package.
You can read more about it [here](https://doc.rust-lang.org/cargo/reference/build-scripts.html).
The `build.rs` file execute the llvm tools you installed with `rustup component add llvm-tools-preview`
in this order:

* Check size of .text section of the kernel if it's too small throw an error
```bash
llvm-size "../../target/x86_64-os/debug/svm_kernel"
```

* Strip debug symbols from kernel to make loading faster
```bash
llvm-objcopy "--strip-debug" "../../target/x86_64-os/debug/svm_kernel" "target/x86_64-bootloader/debug/build/bootloader-c8df27c930d8f65a/out/kernel_stripped-svm_kernel"
```
* Rename the .data section to .kernel in the stripped kernel.
 Objcopy when using `--binary-architecture` flag creates three synthetic symbols
 `_binary_objfile_start`, `_binary_objfile_end`, `_binary_objfile_size.`. 
These symbols use the project / binary name which is why we have to rename them to something more generic
to be able to reference them.
```bash
llvm-objcopy "-I" "binary" "-O" "elf64-x86-64" "--binary-architecture=i386:x86-64" "--rename-section" ".data=.kernel" "--redefine-sym" "_binary_kernel_stripped_svm_kernel_start=_kernel_start_addr" "--redefine-sym" "_binary_kernel_stripped_svm_kernel_end=_kernel_end_addr" "--redefine-sym" "_binary_kernel_stripped_svm_kernel_size=_kernel_size" "target/x86_64-bootloader/debug/build/bootloader-c8df27c930d8f65a/out/kernel_stripped-svm_kernel" "target/x86_64-bootloader/debug/build/bootloader-c8df27c930d8f65a/out/kernel_bin-svm_kernel.o"
```
* Now create a static library out of it
```bash
llvm-ar "crs" "bootloader/target/x86_64-bootloader/debug/build/bootloader-c8df27c930d8f65a/out/libkernel_bin-svm_kernel.a" "target/x86_64-bootloader/debug/build/bootloader-c8df27c930d8f65a/out/kernel_bin-svm_kernel.o"
```
Afterwards `build.rs` tells cargo to use the newly created static library to link against your kernel, with the help of the linker script everything gets placed correctly in the
resulting elf file.

The `build.rs` has another function. It preemptively pads the elf kernel executable with zeros. This way, the bootloader does not have to remap the kernel executable but can just jump into the entry function. Also this way the on disk kernel.elf file is the same as the memory mapped version. 
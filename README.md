## Description
HPC kernel written in Rust.

## Setup & Debug Build
Clone the repo with submodules:
```
$ git clone --recursive <url>
```

Install the rust nightly toolchain:
```
nightly-x86_64-unknown-linux-gnu (default)
rustc 1.49.0-nightly (91a79fb29 2020-10-07)
```

Install the dependencies listed in `shell.nix` or execute
`nix-shell shell.nix` if on NixOS.

Install cargo dependencies:
```
$ cargo install bootimage
```

Run in qemu with:
If you are on intel change the run flags in Cargo.toml and delete "+svm" etc.
```
$ cargo run
```
Close the instance with CTRL+A,X

Built on filechange:
```
$ cd svm_kernel
$ ./watch.sh
```

## Release build:
Edit the file `Cargo.toml` and change `build-command` to `["build", "--release"]`
Then execute `cargo bootimage`

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
$ rust-gdb target/x86_64-os/debug/svm_kernel -ex "target remote:1234"
```

You have to use `hb` instead of `b` in gdb when using qemu-kvm. If not the breakpoints get ignored.


## Run tests
To execute tests run:
```
$ cargo test
```





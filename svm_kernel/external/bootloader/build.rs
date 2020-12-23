
fn main() {
    use std::{
        env,
        path::{PathBuf},
        process::{self, Command},
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // get access to llvm tools shipped in the llvm-tools-preview rustup component
    let llvm_tools = match llvm_tools::LlvmTools::new() {
        Ok(tools) => tools,
        Err(llvm_tools::Error::NotFound) => {
            eprintln!("Error: llvm-tools not found");
            eprintln!("Maybe the rustup component `llvm-tools-preview` is missing?");
            eprintln!("  Install it through: `rustup component add llvm-tools-preview`");
            process::exit(1);
        }
        Err(err) => {
            eprintln!("Failed to retrieve llvm-tools component: {:?}", err);
            process::exit(1);
        }
    };

    // create an archive for linking
    let ar = llvm_tools
        .tool(&llvm_tools::exe("llvm-ar"))
        .unwrap_or_else(|| {
            eprintln!("Failed to retrieve llvm-ar component");
            eprint!("This component is available since nightly-2019-03-29,");
            eprintln!("so try updating your toolchain if you're using an older nightly");
            process::exit(1);
        });

    // ==== Build nasm to binary blob ====
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let stage_nasm = PathBuf::new().join(manifest_dir).join("src/stage_0.nasm");
    let stage_o = out_dir.join("stage_0.o");
    let _stage_bin = out_dir.join("stage_0.bin");
    let stage_a = out_dir.join("libstage_0.a");
    let mut cmd = Command::new("nasm");
    cmd.arg("-f");
    cmd.arg("elf64");
    cmd.arg(stage_nasm);
    cmd.arg("-o");
    cmd.arg(&stage_o);
    let res = cmd.output().unwrap();
    if ! res.status.success() {
        let err = std::str::from_utf8(&res.stderr).unwrap();
        println!("cargo:warning=nasm err: {}", err);
        panic!("Nasm failed to run");
    }

    // ==== Create a static lib out of it ====
    let mut cmd = Command::new(&ar);
    cmd.arg("crs");
    cmd.arg(&stage_a);
    cmd.arg(&stage_o);
    let res = cmd.output().unwrap();
    if ! res.status.success() {
        let err = std::str::from_utf8(&res.stderr).unwrap();
        println!("cargo:warning=nasm err: {}", err);
        panic!("ar failed to run");
    }

    // ==== Tell rustc to link against it ====
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!(
        "cargo:rustc-link-lib=static=stage_0"
    );
    println!("cargo:warning={}", out_dir.display());


    println!("cargo:rerun-if-changed=stage_0.nasm");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=linker.ld");
}

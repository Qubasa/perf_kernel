use clap::{App, Arg, ArgMatches, SubCommand};
use log::*;
use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process,
};
use std::{fs::OpenOptions, io::Write};

mod config;
mod run;
fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Trace)
        .with_timestamps(false)
        .init()
        .unwrap();
    log::set_max_level(LevelFilter::Info);

    let matches = App::new("Glue gun")
        .author("Luis Hebendanz <luis.nixos@gmail.com")
        .about("Glues together a rust bootloader and kernel to generate a bootable ISO file")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .help("Enables verbose mode")
                .takes_value(false),
        )
        .subcommand(
            SubCommand::with_name("run")
                .about("Builds and runs the ISO file")
                .arg(
                    Arg::with_name("grub")
                        .help("Encapsulates your kernel with grub 2")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("verbose")
                        .help("Enables verbose mode")
                        .short("v")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("debug")
                        .help("Runs kernel with debug run command")
                        .short("d")
                        .takes_value(false),
                ),
        )
        .get_matches();

    if matches.is_present("verbose") {
        log::set_max_level(LevelFilter::Debug);
    }

    if let Some(matches) = matches.subcommand_matches("run") {
        if matches.is_present("verbose") {
            log::set_max_level(LevelFilter::Debug);
        }
        debug!("Args: {:?}", std::env::args());

        run(matches);
    }
}

fn run(matches: &ArgMatches) {
    /*
        Where do these environment variables come from?
        https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    */
    let kernel_manifest;
    let kernel_crate;

    let config;
    {
        kernel_manifest = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
        kernel_crate = kernel_manifest
            .parent()
            .expect("Kernel directory does not have a parent dir")
            .to_path_buf();
        debug!("Manifest path: {:#?}", kernel_manifest);

        config = config::read_config(&kernel_manifest).unwrap();
    }

    let target_dir;
    let is_verbose = matches.is_present("verbose");
    let is_release;
    let is_test;
    let kernel;
    {
        kernel = Path::new(matches.value_of("grub").expect("missing executable path"));
        target_dir = kernel
            .parent()
            .expect("Target executable does not have a parent directory")
            .to_path_buf();
        is_release = target_dir.iter().last().unwrap() == OsStr::new("release");

        let is_doctest = target_dir
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rustdoctest");
        is_test = is_doctest || target_dir.ends_with("deps");
    }
    debug!("Building in release mode? {}", is_release);
    debug!("Running a test? {}", is_test);

    let bootloader_manifest;
    let bootloader_crate;
    {
        let metadata = cargo_metadata::MetadataCommand::new()
            .manifest_path(kernel_manifest.as_path())
            .exec()
            .unwrap();

        let kernel_pkg = metadata
            .packages
            .iter()
            .find(|p| p.manifest_path == kernel_manifest)
            .expect("Couldn't find package with same manifest as kernel in metadata");

        let bootloader_name = kernel_pkg
            .dependencies
            .iter()
            .find(|d| d.rename.as_ref().unwrap_or(&d.name) == "bootloader")
            .expect("Couldn't find needed dependencie 'bootloader' in kernel")
            .name
            .clone();

        let bootloader_pkg = metadata
            .packages
            .iter()
            .find(|p| p.name == bootloader_name)
            .unwrap();

        bootloader_manifest = bootloader_pkg
            .manifest_path
            .clone()
            .to_path_buf()
            .into_std_path_buf();
        bootloader_crate = bootloader_manifest.parent().unwrap();
    }
    debug!("Bootloader manifest: {:?}", bootloader_manifest);
    debug!("Bootloader crate: {:?}", bootloader_crate);

    let merged_exe;
    {
        let mut full_kernel_path = kernel_crate.clone();
        full_kernel_path.push(kernel);
        let env_vars = [("KERNEL", full_kernel_path.to_str().unwrap())];
        let features = ["binary"];
        let exes = cargo_build(
            bootloader_crate,
            &config,
            is_release,
            is_verbose,
            Some(&features),
            Some(&env_vars),
        );

        if exes.len() != 1 {
            panic!("bootloader generated more then one executable");
        }

        let exe = &exes[0];
        let dst = exe.parent().unwrap().join(kernel.file_name().unwrap());
        std::fs::rename(exe, &dst).expect("Failed to rename bootloader executable");

        merged_exe = dst;
    }
    debug!("Merged executable: {:?}", merged_exe);

    let iso_img;
    {
        let kernel_name = merged_exe.file_stem().unwrap().to_str().unwrap();
        iso_img = target_dir.join(format!("bootimage-{}.iso", kernel_name));
        let iso_dir = target_dir.join("isofiles");

        println!("Iso for {} -> {}", kernel_name, iso_img.to_str().unwrap());

        glue_grub(&iso_dir, &iso_img, &merged_exe);
    }

    run::run(config, &iso_img, is_test, matches.is_present("debug")).unwrap();
}

fn glue_grub(iso_dir: &PathBuf, iso_img: &PathBuf, executable: &PathBuf) {
    match std::fs::create_dir(iso_dir) {
        Ok(_) => (),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
            } else {
                panic!(
                    "{} Failed to create iso dir {}",
                    e,
                    iso_dir.to_str().unwrap()
                );
            }
        }
    };

    let grub_dir = iso_dir.join("boot/grub");
    match std::fs::create_dir_all(&grub_dir) {
        Ok(_) => (),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
            } else {
                panic!(
                    "{} Failed to create iso dir {}",
                    e,
                    iso_dir.to_str().unwrap()
                );
            }
        }
    };

    let mut grubcfg = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&grub_dir.join("grub.cfg"))
        .unwrap();

    grubcfg
        .write(
            r#"
            set timeout=0
            set default=0

            menuentry "kernel" {
                multiboot2 /boot/kernel.elf
                boot
            }
            "#
            .as_bytes(),
        )
        .unwrap();

    std::fs::copy(executable, iso_dir.join("boot/kernel.elf")).unwrap();

    let mut cmd = process::Command::new("grub-mkrescue");
    cmd.arg("-o").arg(iso_img);
    cmd.arg(iso_dir);

    let output = cmd.output().expect("Failed to build bootloader crate");
    if !output.status.success() {
        panic!(
            "Failed to build grub image: {}",
            std::str::from_utf8(&output.stderr).unwrap()
        );
    }
}

fn cargo_build(
    target_crate: &Path,
    config: &config::Config,
    is_release: bool,
    is_verbose: bool,
    features: Option<&[&str]>,
    env: Option<&[(&str, &str)]>,
) -> Vec<PathBuf> {
    let mut executables = Vec::new();

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut cmd = process::Command::new(&cargo);
    cmd.current_dir(target_crate);
    if let Some(env) = env {
        for (key, val) in env {
            cmd.env(key, val);
        }
        debug!("Env vars: {:?}", env);
    }
    cmd.args(config.build_command.clone());
    if let Some(features) = features {
        cmd.arg(format!(
            "--features={}",
            features
                .iter()
                .fold("".to_string(), |acc, x| format!("{},{}", acc, x))
        ));
    }

    if is_release {
        cmd.arg("--release");
    }

    if is_verbose {
        cmd.arg("-vv");
    }

    cmd.stdout(process::Stdio::inherit());
    cmd.stderr(process::Stdio::inherit());
    debug!("Running command: {:#?}", cmd);

    let output = cmd.output().expect("Failed to build bootloader crate");
    if !output.status.success() {
        panic!("Failed to build bootloader crate");
    }

    // Redo build just to parse out json and get executable paths
    cmd.arg("--message-format").arg("json");
    cmd.stderr(process::Stdio::piped());
    cmd.stdout(process::Stdio::piped());
    let output = cmd.output().expect("Failed to build bootloader crate");
    if !output.status.success() {
        panic!(
            "Failed to build bootloader crate: {}",
            std::str::from_utf8(&output.stderr).unwrap()
        );
    }
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let mut artifact = json::parse(line).expect("Failed parsing json from cargo");
        if let Some(executable) = artifact["executable"].take_string() {
            executables.push(PathBuf::from(executable));
        }
    }
    executables
}

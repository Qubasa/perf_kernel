#[cfg(not(feature = "binary"))]
fn main() {}

#[cfg(feature = "binary")]
fn main() {
    use std::{
        env,
        path::PathBuf,
        process::{self, Command},
    };

    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "x86_64" {
        process::exit(1);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    let kernel = PathBuf::from(match env::var("KERNEL") {
        Ok(kernel) => kernel,
        Err(_) => {
            eprintln!(
                "The KERNEL environment variable must be set for building the bootloader.\n\n\
                     If you use `bootimage` for building you need at least version 0.7.0. You can \
                     update `bootimage` by running `cargo install bootimage --force`."
            );
            process::exit(1);
        }
    });

    let kernel_file_name = kernel
        .file_name()
        .expect("KERNEL has no valid file name")
        .to_str()
        .expect("kernel file name not valid utf8");

    // check that the kernel file exists
    assert!(
        kernel.exists(),
        format!("KERNEL does not exist: {}", kernel.display())
    );

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

    // Check that kernel executable has code in it
    let llvm_size = llvm_tools
        .tool(&llvm_tools::exe("llvm-size"))
        .expect("llvm-size not found in llvm-tools");
    {
        let mut cmd = Command::new(llvm_size);
        cmd.arg(&kernel);
        let output = cmd.output().expect("failed to run llvm-size");
        let output_str = String::from_utf8_lossy(&output.stdout);
        let second_line_opt = output_str.lines().skip(1).next();
        let second_line = second_line_opt.expect("unexpected llvm-size line output");
        let text_size_opt = second_line.split_ascii_whitespace().next();
        let text_size = text_size_opt.expect("unexpected llvm-size output");
        if text_size == "0" {
            panic!("Kernel executable has an empty text section. Perhaps the entry point was set incorrectly?\n\n\
            Kernel executable at `{}`\n", kernel.display());
        }
    }

    // Strip debug symbols from kernel for faster loading
    let stripped_kernel_file_name = format!("kernel_stripped-{}", kernel_file_name);
    let stripped_kernel = out_dir.join(&stripped_kernel_file_name);
    let objcopy = llvm_tools
        .tool(&llvm_tools::exe("llvm-objcopy"))
        .expect("llvm-objcopy not found in llvm-tools");
    {
        let mut cmd = Command::new(&objcopy);
        cmd.arg("--strip-all");
        cmd.arg(&kernel);
        cmd.arg(&stripped_kernel);
        let exit_status = cmd
            .status()
            .expect("failed to run objcopy to strip debug symbols");
        if !exit_status.success() {
            eprintln!("Error: Stripping debug symbols failed");
            process::exit(1);
        }
    }

    pad_kernel(&stripped_kernel);

    let kernel_obj = out_dir.join(format!("kernel_bin-{}.o", kernel_file_name));
    {
        let stripped_kernel_name_replaced = stripped_kernel_file_name
            .replace('-', "_")
            .replace('.', "_")
            .replace("/", "_");

        // wrap
        let mut cmd = Command::new(&objcopy);
        cmd.arg("-I").arg("binary");
        cmd.arg("-O").arg("elf32-i386");
        cmd.arg("--binary-architecture=i386:x86-64");
        cmd.arg("--rename-section").arg(".data=.kernel");
        cmd.arg("--redefine-sym").arg(format!(
            "_binary_{}_start=_kernel_start_addr",
            stripped_kernel_name_replaced
        ));
        cmd.arg("--redefine-sym").arg(format!(
            "_binary_{}_end=_kernel_end_addr",
            stripped_kernel_name_replaced
        ));
        cmd.arg("--redefine-sym").arg(format!(
            "_binary_{}_size=_kernel_size",
            stripped_kernel_name_replaced
        ));
        cmd.current_dir(&out_dir);
        cmd.arg(&stripped_kernel_file_name);
        cmd.arg(&kernel_obj);

        let exit_status = cmd.status().expect("failed to run objcopy");
        if !exit_status.success() {
            eprintln!("Error: Running objcopy failed");
            process::exit(1);
        }
    }

    // Create an archive for linking
    let kernel_archive = out_dir.join(format!("libkernel_bin-{}.a", kernel_file_name));
    {
        let ar = llvm_tools
            .tool(&llvm_tools::exe("llvm-ar"))
            .unwrap_or_else(|| {
                eprintln!("Failed to retrieve llvm-ar component");
                eprint!("This component is available since nightly-2019-03-29,");
                eprintln!("so try updating your toolchain if you're using an older nightly");
                process::exit(1);
            });
        let mut cmd = Command::new(ar);
        cmd.arg("crs");
        cmd.arg(&kernel_archive);
        cmd.arg(&kernel_obj);
        let exit_status = cmd.status().expect("failed to run ar");
        if !exit_status.success() {
            eprintln!("Error: Running ar failed");
            process::exit(1);
        }
    }

    // Give build instructions to rustc
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!(
        "cargo:rustc-link-lib=static=kernel_bin-{}",
        kernel_file_name
    );

    // Display tmp file directory as warning
    println!("cargo:warning={}", out_dir.display());
    println!("cargo:rerun-if-env-changed=KERNEL");
    println!("cargo:rerun-if-changed={}", kernel.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=linker.ld");
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Elf32Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32,
    e_phoff: u32,
    e_shoff: u32,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Elf64Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Elf64_Shdr {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd)]
#[repr(C, packed)]
struct Elf64_Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

impl Ord for Elf64_Phdr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        unsafe { self.p_vaddr.cmp(&other.p_vaddr) }
    }
}

use std::fmt;
impl fmt::Display for Elf64_Phdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { write!(f, "Virtual Address: {:#x}", self.p_vaddr) }
    }
}

impl fmt::Debug for Elf64_Phdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { write!(f, "Virtual Address: {:#x}", self.p_vaddr) }
    }
}

/*
 * Pads the kernel ELF file on disk to match it's memory representation by inserting zeros
 * where necessary.
 */
#[cfg(feature = "binary")]
fn pad_kernel(kernel: &std::path::PathBuf) {
    use std::convert::{TryFrom, TryInto};
    use std::io::{Read, Seek, Write};

    // Read file to vec
    let mut buf = Vec::<u8>::new();
    let mut kernel_fd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&kernel)
        .expect("Could not open a filedescriptor to kernel");
    kernel_fd
        .read_to_end(&mut buf)
        .expect("Could not read kernel file");

    /*
     * Parse ELF header
     */
    let header;
    {
        const HEADER_SIZE: usize = std::mem::size_of::<Elf64Header>();
        let header_buf: [u8; HEADER_SIZE] = buf[..HEADER_SIZE].try_into().expect("Failed try into");
        header = unsafe { std::mem::transmute::<&[u8; HEADER_SIZE], &Elf64Header>(&header_buf) };
        let magic = [
            0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        if header.e_ident != magic {
            for i in header.e_ident.iter() {
                eprintln!("{:#x} ", i);
            }
            eprintln!("Kernel binary has incorrect ELF magic");
            std::process::exit(1);
        }
        if header.e_phoff == 0 {
            panic!("Kernel ELF does not have a program header table");
        }

        if header.e_shoff == 0 {
            panic!("Kernel ELF does not have a section header table");
        }
    }

    /*
     * Parse section header and zero .bss section
     */
    {
        const SHEADER_SIZE: usize = std::mem::size_of::<Elf64_Shdr>();

        let mut bss_sec = None;
        {
            let s = &buf[header.e_shoff as usize
                ..header.e_shoff as usize + SHEADER_SIZE * header.e_shnum as usize];

            let (prefix, sections, suffix) = unsafe { s.align_to::<Elf64_Shdr>() };
            if !prefix.is_empty() || !suffix.is_empty() {
                panic!("Misaligned sheader read");
            }

            let str_table_sec = sections[header.e_shstrndx as usize];
            let offset = str_table_sec.sh_offset as usize;
            let str_table = &buf[offset..offset + str_table_sec.sh_size as usize];
            for section in sections {
                let name = core::str::from_utf8(
                    &str_table[section.sh_name as usize..]
                        .split(|&c|{  c == 0})
                        .next().unwrap(),
                )
                .unwrap();
                if name == ".bss" {
                    bss_sec = Some(section.clone());
                }
            }
        }

        if let Some(section) = bss_sec {
            let offset = section.sh_offset as usize;
            let bss = &mut buf[offset..offset+section.sh_size as usize];
            for i in bss {
                *i = 0;
            }
        }else{
            panic!("Kernel does not have a .bss section");
        }
    }

    /*
     * Parse program header and start padding segments
     */
    const PHEADER_SIZE: usize = std::mem::size_of::<Elf64_Phdr>();
    let mut load_segments = Vec::new();
    {
        let phoff = header.e_phoff.try_into().unwrap();
        let mut header_buf: [u8; PHEADER_SIZE] =
            buf[phoff..phoff + PHEADER_SIZE].try_into().unwrap();
        let pheader =
            unsafe { std::mem::transmute::<&[u8; PHEADER_SIZE], &Elf64_Phdr>(&header_buf) };

        let mut i = 0;
        #[allow(unused_assignments)]
        while i < header.e_phnum.into() {
            let window = phoff + PHEADER_SIZE * i;
            header_buf = buf[window..window + PHEADER_SIZE].try_into().unwrap();

            if pheader.p_type == 1 {
                // LOAD Segment
                load_segments.push(pheader.clone());
            }
            i += 1;
        }
        load_segments.sort();

        // Check that the virt address order correlates with the file offset order
        for (seg, seg_next) in load_segments.iter().zip(load_segments.iter().skip(1)) {
            if seg.p_offset >= seg_next.p_offset {
                eprintln!(
                    "Segments sorted by their virtual address have a different file offset order."
                );
                eprintln!("Padding is impossible");
                std::process::exit(1);
            }
        }

        // Check that program header is included in first LOAD segment
        if load_segments.len() > 1 && load_segments[1].p_offset < header.e_phoff {
            unsafe {
                eprintln!("Programm header comes after LOAD2 segment. This is not supported.");
                eprintln!("Program header addr: {:#x}", header.e_phoff);
                eprintln!("LOAD2 offset: {:#x}", load_segments[0].p_offset);
            }
            std::process::exit(1);
        }

        // Check that first load segment has virtual address 0x200000
        if load_segments.first().unwrap().p_vaddr != 0x200000 {
            panic!("Base address (first load segment) has to be 0x200000 = 2Mb");
        }
    }

    // Pad load segments with zeros
    let mut already_padded: usize = 0;
    for (i, (seg, seg_next)) in load_segments
        .iter()
        .zip(load_segments.iter().skip(1))
        .enumerate()
    {
        let vdiff = (isize::try_from(seg_next.p_vaddr).unwrap())
            .checked_sub(isize::try_from(seg.p_vaddr).unwrap())
            .unwrap();
        let mut pad_size =
            usize::try_from((vdiff - isize::try_from(seg.p_filesz).unwrap()).abs()).unwrap();
        eprintln!("Pad size: {:#x}", pad_size);

        // Often times segments have spaces in between each other that do not belong to any segment
        // These spaces have to be subtracted from the padding
        let sub_align =
            seg_next.p_offset as isize - (seg.p_filesz as isize + seg.p_offset as isize);
        unsafe {
            eprintln!(
                "LOAD{}: In file align: {:#x} next segment offset: {:#x}",
                i,
                seg_next.p_offset,
                seg.p_filesz as isize + seg.p_offset as isize
            );
        }
        eprintln!("Subtract align: {:#x}", sub_align.abs());
        pad_size -= sub_align.abs() as usize;

        let index: usize = usize::try_from(seg.p_offset).unwrap()
            + usize::try_from(seg.p_filesz).unwrap()
            + already_padded
            + 1;
        eprintln!("Padding from: {:#x} to {:#x}", index, index + pad_size);

        for _ in 0..pad_size {
            buf.insert(index, 0); // TODO: Make more efficient
        }

        // Update program header with new file size etc.
        let p = &mut buf[header.e_phoff as usize
            ..header.e_phoff as usize + PHEADER_SIZE * header.e_phnum as usize];

        let p_iter = p.chunks_mut(PHEADER_SIZE);

        for i in p_iter {
            let (prefix, pheaders, suffix) = unsafe { i.align_to_mut::<Elf64_Phdr>() };
            let pheader = &mut pheaders[0];
            if !prefix.is_empty() || !suffix.is_empty() {
                panic!("Misaligned pheader read");
            }
            if pheader.p_offset == seg.p_offset {
                let pad_size = u64::try_from(pad_size).unwrap();
                pheader.p_filesz += pad_size;
                pheader.p_memsz += pad_size;
                pheader.p_offset += u64::try_from(already_padded).unwrap();
            }
        }

        // Sum of all applied pad sizes
        already_padded += pad_size;
    }

    println!("cargo:warning=Total padding: {:#x}", already_padded);
    eprintln!("Writing to file");
    kernel_fd
        .seek(std::io::SeekFrom::Start(0))
        .expect("Seeking failed");
    kernel_fd
        .write_all(buf.as_slice())
        .expect("Failed to pad kernel executable");
    kernel_fd.sync_all().unwrap();
}

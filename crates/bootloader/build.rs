#[cfg(not(feature = "binary"))]
fn main() {}

#[cfg(feature = "binary")]
fn main() {
    use std::{
        env,
        path::PathBuf,
        process::{self, Command},
    };

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if arch != "x86" {
        eprintln!("Building binary in incorrect architecture: {}", arch);
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

    eprintln!("Original kernel path: {:#?}", kernel.clone());

    let kernel_file_name = kernel
        .file_name()
        .expect("KERNEL has no valid file name")
        .to_str()
        .expect("kernel file name not valid utf8");

    // check that the kernel file exists
    assert!(
        kernel.exists(),
        "KERNEL does not exist: {}",
        kernel.display()
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
        println!("Executing:\n {:#?}", cmd);
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
        cmd.arg("--strip-debug");
        cmd.arg(&kernel);
        cmd.arg(&stripped_kernel);
        println!("Executing:\n {:#?}", cmd);
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
        println!("Executing:\n {:#?}", cmd);
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
        println!("Executing:\n {:#?}", cmd);
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
    println!("Artifacts dir: {}", out_dir.display());
    println!("cargo:rerun-if-env-changed=KERNEL");
    println!("cargo:rerun-if-changed={}", kernel.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=linker.ld");
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

#[derive(Clone, Copy, Eq, PartialEq)]
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

use core::ptr::addr_of;
use core::ptr::read_unaligned;
impl Ord for Elf64_Phdr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        unsafe {
            let x = read_unaligned(addr_of!(other.p_vaddr));
            read_unaligned(addr_of!(self.p_vaddr)).cmp(&x)
        }
    }
}

impl PartialOrd for Elf64_Phdr {
    fn partial_cmp(&self, other: &Elf64_Phdr) -> Option<Ordering> {
        unsafe {
            let x = read_unaligned(addr_of!(other.p_vaddr));
            let r = read_unaligned(addr_of!(self.p_vaddr)).cmp(&x);
            Some(r)
        }
    }
}

use std::{cmp::Ordering, fmt};
impl fmt::Display for Elf64_Shdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            writeln!(
                f,
                "\nsh_addr: {:#x}",
                read_unaligned(addr_of!(self.sh_addr))
            )?;
            writeln!(
                f,
                "sh_offset: {:#x}",
                read_unaligned(addr_of!(self.sh_offset))
            )?;
            writeln!(f, "sh_size: {:#x}", read_unaligned(addr_of!(self.sh_size)))
        }
    }
}
impl fmt::Display for Elf64_Phdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "Virtual Address: {:#x} Offset: {:#x}",
                read_unaligned(addr_of!(self.p_vaddr)),
                read_unaligned(addr_of!(self.p_offset))
            )
        }
    }
}

impl fmt::Debug for Elf64_Phdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "Virtual Address: {:#x} Offset: {:#x}",
                read_unaligned(addr_of!(self.p_vaddr)),
                read_unaligned(addr_of!(self.p_offset))
            )
        }
    }
}

#[cfg(feature = "binary")]
fn apply_type<T: Sized>(offset: u64, num_types: u64, buf: &[u8]) -> Result<&[T], ApplyTypeError> {
    use std::convert::TryFrom;
    use ApplyTypeError::*;
    let type_size: usize = std::mem::size_of::<T>();
    let offset = usize::try_from(offset).map_err(|_| UsizeTransform)?;
    let num_types = usize::try_from(num_types).map_err(|_| UsizeTransform)?;
    let s = &buf[offset..offset + type_size * num_types];
    let (prefix, headers, suffix) = unsafe { s.align_to::<T>() };
    if !prefix.is_empty() || !suffix.is_empty() {
        return Err(MisalignedRead(std::any::type_name::<T>()));
    }
    Ok(headers)
}

#[cfg(feature = "binary")]
fn apply_type_mut<T: Sized>(
    offset: u64,
    num_types: u64,
    buf: &mut [u8],
) -> Result<&mut [T], ApplyTypeError> {
    use std::convert::TryFrom;
    use ApplyTypeError::*;
    let type_size: usize = std::mem::size_of::<T>();
    let offset = usize::try_from(offset).map_err(|_| UsizeTransform)?;
    let num_types = usize::try_from(num_types).map_err(|_| UsizeTransform)?;
    let s = &mut buf[offset..offset + type_size * num_types];
    let (prefix, headers, suffix) = unsafe { s.align_to_mut::<T>() };
    if !prefix.is_empty() || !suffix.is_empty() {
        return Err(MisalignedRead(std::any::type_name::<T>()));
    }
    Ok(headers)
}

#[cfg(feature = "binary")]
fn addr_to_seg_map(addr: u64, segments: &Vec<Elf64_Phdr>) -> Option<usize> {
    for (i, seg) in segments.iter().enumerate() {
        let low_bar = seg.p_offset;
        let high_bar = seg.p_offset + seg.p_memsz;
        if addr >= low_bar && addr < high_bar {
            return Some(i);
        }
        // eprintln!(
        //     "addr({:#x}) >= low_bar({:#x}) = {}",
        //     addr,
        //     low_bar,
        //     addr >= low_bar
        // );
        // eprintln!(
        //     "&& addr({:#x})  < high_bar({:#x}) = {}",
        //     addr,
        //     high_bar,
        //     addr < high_bar
        // );
    }
    None
}

#[cfg(feature = "binary")]
#[derive(Debug)]
enum ApplyTypeError {
    UsizeTransform,
    MisalignedRead(&'static str),
}

/*
 * Pads the kernel ELF file on disk to match it's memory representation by inserting zeros
 * where necessary.
 */
#[cfg(feature = "binary")]
fn pad_kernel(kernel: &std::path::PathBuf) {
    use std::convert::TryFrom;
    use std::io::{Read, Seek, Write};

    eprintln!(
        "Padding kernel file: {:#?}",
        kernel.clone().into_os_string()
    );

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
    let mut header;
    {
        let headers = apply_type::<Elf64Header>(0, 1, &buf).unwrap();
        header = headers[0].clone();

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
     * Parse program header and save load segments in vec
     */
    let mut segments = Vec::<Elf64_Phdr>::new();
    let mut load_segments = Vec::<Elf64_Phdr>::new();
    {
        let pheaders =
            apply_type::<Elf64_Phdr>(header.e_phoff, header.e_phnum.into(), &buf).unwrap();

        for pheader in pheaders {
            // LOAD Segment
            segments.push(pheader.clone());
            if pheader.p_type == 1 {
                load_segments.push(pheader.clone());
            }
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
        if load_segments.iter().count() > 1
            && load_segments.iter().nth(1).unwrap().p_offset <= header.e_phoff
        {
            unsafe {
                eprintln!("Programm header comes after LOAD2 segment. This is not supported.");
                eprintln!(
                    "Program header addr: {:#x}",
                    read_unaligned(addr_of!(header.e_phoff))
                );
                eprintln!(
                    "LOAD2 offset: {:#x}",
                    read_unaligned(addr_of!(load_segments.iter().nth(0).unwrap().p_offset))
                );
            }
            std::process::exit(1);
        }

        unsafe {
            // Check that first load segment has virtual address 0x200000
            if read_unaligned(addr_of!(load_segments.iter().nth(0).unwrap().p_vaddr)) != 0x200000 {
                panic!("Base address (first load segment) has to be 0x200000 = 2Mb");
            }
        }
    }

    // Pad load segments with zeros
    let mut already_padded_vec: Vec<usize> = Vec::new();
    for (_i, (seg, seg_next)) in load_segments
        .iter()
        .zip(load_segments.iter().skip(1))
        .enumerate()
    {
        let already_padded: usize = already_padded_vec.iter().sum();
        let vdiff = usize::try_from(
            std::cmp::max(seg.p_vaddr, seg_next.p_vaddr)
                - std::cmp::min(seg.p_vaddr, seg_next.p_vaddr),
        )
        .unwrap();

        let mut pad_size = vdiff
            .checked_sub(usize::try_from(seg.p_filesz).unwrap())
            .unwrap();

        // Often times segments have spaces in between each other that do not belong to any segment
        // These spaces have to be subtracted from the padding
        let a = seg.p_filesz + seg.p_offset;
        let b = seg_next.p_offset;
        let sub_align = usize::try_from(std::cmp::max(a, b) - std::cmp::min(a, b)).unwrap();

        pad_size -= sub_align;

        let index: usize = usize::try_from(seg.p_offset).unwrap()
            + usize::try_from(seg.p_filesz).unwrap()
            + already_padded
            + 1;

        eprintln!("Start padding at: {:#x} with {:#x} bytes", index, pad_size);

        let zero = std::iter::repeat(0).take(pad_size);
        buf.splice(index..index, zero);

        // Sum of all applied pad sizes
        already_padded_vec.push(pad_size);
    }

    // Pad last load segment too
    {
        let last = load_segments.last().unwrap();
        let pad_size = usize::try_from(last.p_memsz - last.p_filesz).unwrap();
        let already_padded: usize = already_padded_vec.iter().sum();
        let index: usize = usize::try_from(last.p_offset).unwrap()
            + usize::try_from(last.p_filesz).unwrap()
            + already_padded;

        eprintln!("Padding last load segment at {:#x} by: {:#x} bytes", index, pad_size);

        let zero = std::iter::repeat(0).take(pad_size);
        buf.splice(index..index, zero);

        already_padded_vec.push(pad_size);
    }

    // Update program header with new file size etc.
    {
        let pheaders =
            apply_type_mut::<Elf64_Phdr>(header.e_phoff, header.e_phnum.into(), &mut buf).unwrap();

        for pheader in pheaders {
            if pheader.p_type == 1 {
                let idx = addr_to_seg_map(pheader.p_offset, &load_segments).unwrap();
                let seg = &load_segments[idx];
                let pad_size = already_padded_vec[idx];
                let already_padded = already_padded_vec[..idx].iter().sum::<usize>();

                if pheader.p_offset == seg.p_offset {
                    let pad_size = u64::try_from(pad_size).unwrap();
                    pheader.p_filesz += pad_size;
                    pheader.p_memsz += pad_size;
                    pheader.p_offset += u64::try_from(already_padded).unwrap();
                }
            }
        }
    }

    // Update header offsets
    {
        let headers = apply_type_mut::<Elf64Header>(0, 1, &mut buf).unwrap();
        let header_ref = &mut headers[0];

        let last_seg = load_segments.last().unwrap();
        if header_ref.e_shoff < last_seg.p_offset + last_seg.p_filesz {
            unsafe {
                eprintln!(
                    "Section header offset: {:#x} last segment: {:#x}",
                    read_unaligned(addr_of!(header_ref.e_shoff)),
                    last_seg.p_offset + last_seg.p_filesz
                );
            }
            panic!("Section header table is not at the end of ELF file.");
        }
        header_ref.e_shoff += already_padded_vec.iter().sum::<usize>() as u64;

        // Update header clone
        header = header_ref.clone();
    }

    // Update section offsets
    {
        let sections =
            apply_type_mut::<Elf64_Shdr>(header.e_shoff, header.e_shnum.into(), &mut buf).unwrap();

        for section in sections {
            // If section does not map to load_segment what should we do?
            let idx = if let Some(idx) = addr_to_seg_map(section.sh_offset, &load_segments) {
                idx
            } else {
                // Assume that it is at the end of the file
                eprintln!("Section: {:#x?}", section);
                eprintln!("Section does not map to load segment");
                if section.sh_offset > load_segments.last().unwrap().p_offset {
                    load_segments.len() - 1
                } else {
                    panic!(
                        "Section is before last load segment but does not map to any load segment"
                    );
                }
            };
            let already_padded = already_padded_vec[..idx].iter().sum::<usize>();

            section.sh_offset += already_padded as u64;

            // Edge case: If section behind last load segment
            let last = load_segments.last().unwrap();
            if section.sh_offset > last.p_offset + last.p_filesz {
                section.sh_offset += *already_padded_vec.last().unwrap() as u64;
            }
        }
    }

    /*
     * Parse section header and zero .bss section
     */
    {
        let mut bss_sec = None;
        {
            let sections =
                apply_type::<Elf64_Shdr>(header.e_shoff, header.e_shnum.into(), &buf).unwrap();

            let str_table_sec = sections[header.e_shstrndx as usize];
            let offset = str_table_sec.sh_offset as usize;
            let str_table = &buf[offset..offset + str_table_sec.sh_size as usize];
            for section in sections {
                let name = core::str::from_utf8(
                    &str_table[section.sh_name as usize..]
                        .split(|&c| c == 0)
                        .next()
                        .unwrap(),
                )
                .unwrap();
                if name == ".bss" {
                    bss_sec = Some(section.clone());
                }
            }
        }

        if let Some(section) = bss_sec {
            let offset = section.sh_offset as usize;
            // Edge case: bss section lies outside of the padded file
            if offset + section.sh_size as usize > buf.len() {
                let pad_size = offset + section.sh_size as usize - buf.len();
                let buf_len = buf.len();
                buf.resize(buf_len + pad_size, 0);
            }
            let bss = &mut buf[offset..offset + section.sh_size as usize];
            for i in bss {
                *i = 0;
            }
        } else {
            println!("cargo:warning=Kernel does not have a .bss section");
        }
    }

    println!(
        "Total padding: {} Kb",
        already_padded_vec.iter().sum::<usize>() / 1024
    );
    kernel_fd
        .seek(std::io::SeekFrom::Start(0))
        .expect("Seeking failed");
    kernel_fd
        .write_all(buf.as_slice())
        .expect("Failed to pad kernel executable");
    kernel_fd.sync_all().unwrap();

  
}

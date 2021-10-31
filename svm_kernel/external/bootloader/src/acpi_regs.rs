use core::fmt;
use core::ptr::addr_of;
use core::ptr::read_unaligned;

/// In-memory representation of an RSDP ACPI structure
#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Rsdp {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_addr: u32,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct RsdpExtended {
    pub descriptor: Rsdp,
    pub length: u32,
    pub xsdt_addr: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

/// In-memory representation of an ACPI table header
#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Header {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oemid: [u8; 6],
    pub oem_table_id: u64,
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IoApic {
    pub typ: u8,
    pub length: u8,
    pub id: u8,
    pub res0: u8,
    pub address: u32,
    pub interrupt_base: u32,
}

impl fmt::Debug for IoApic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            write!(
                f,
                "IoApic address: {:#x}",
                read_unaligned(addr_of!(self.address))
            )
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C, packed)]
pub struct LocalApic {
    pub typ: u8,
    pub length: u8,
    pub processor_uid: u8,
    pub id: u8,
    pub flags: u32,
}

impl fmt::Debug for LocalApic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LApic id: {}", self.id)
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IntOverride {
    pub typ: u8,
    pub length: u8,
    pub bus: u8, // always 0
    pub source: u8,
    pub mapped_to: u32,
    pub flags: u16,
}
// TODO: Misaligned reads from packed struct in Debug could cause problems?
impl fmt::Debug for IntOverride {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            write!(
                f,
                "IntOverride src: {} mapped to: {}",
                self.source,
                read_unaligned(addr_of!(self.mapped_to))
            )
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct NonMaskableInts {
    pub typ: u8,
    pub length: u8,
    pub flags: u16,
    pub int_num: u32,
}

impl fmt::Debug for NonMaskableInts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            write!(
                f,
                "Non Maskable Interrupt: {}",
                read_unaligned(addr_of!(self.int_num))
            )
        }
    }
}

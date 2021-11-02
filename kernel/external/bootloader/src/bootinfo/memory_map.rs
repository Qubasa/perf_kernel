use core::fmt;
use core::ops::{Deref, DerefMut};
use core::ptr::addr_of;
use core::ptr::read_unaligned;

const PAGE_SIZE: u64 = 4096;

const MAX_MEMORY_MAP_SIZE: usize = 3840;

/// A map of the physical memory regions of the underlying machine.
#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct MemoryMap {
    entries: [MemoryRegion; MAX_MEMORY_MAP_SIZE],
    // u64 instead of usize so that the structure layout is platform
    // independent
    next_entry_index: u64,
}

#[derive(Debug)]
pub enum PartitionError {
    NotSameRegion(MemoryRegion, MemoryRegion),
    AddrDoesNotExist,
    InvalidRegionType,
    AddrIsNotAligned(u64),
    RegionTypeIsNotUsable(MemoryRegion),
}

#[derive(Debug)]
pub enum RemoveError {
    RegionDoesNotExist,
}

#[doc(hidden)]
#[allow(clippy::new_without_default)]
impl MemoryMap {
    /// Creates empty memory map
    pub const fn new() -> Self {
        MemoryMap {
            entries: [MemoryRegion::empty(); MAX_MEMORY_MAP_SIZE],
            next_entry_index: 0,
        }
    }

    /// Returns true if supplied address is in a usable region
    pub fn get_region_by_addr(&self, addr: u64) -> Option<&MemoryRegion> {
        for i in self.entries[0..self.next_entry_index()].iter() {
            if i.range.intersects(addr) {
                return Some(i);
            }
        }
        None
    }

    pub fn partition_memory_region(
        &mut self,
        start_addr: u64,
        end_addr: u64,
        region_type: MemoryRegionType,
    ) -> Result<[MemoryRegion; 3], PartitionError> {
        if region_type == MemoryRegionType::Empty {
            return Err(PartitionError::InvalidRegionType);
        }

        if start_addr % PAGE_SIZE != 0 {
            return Err(PartitionError::AddrIsNotAligned(start_addr));
        }

        if end_addr % PAGE_SIZE != 0 {
            return Err(PartitionError::AddrIsNotAligned(end_addr));
        }

        let end_region = *self
            .get_region_by_addr(end_addr - 1)
            .ok_or(PartitionError::AddrDoesNotExist)?;
        let region = *self
            .get_region_by_addr(start_addr)
            .ok_or(PartitionError::AddrDoesNotExist)?;
        if region != end_region {
            return Err(PartitionError::NotSameRegion(region, end_region));
        }

        let mem_type = unsafe { read_unaligned(addr_of!(region.region_type)) };
        if mem_type != MemoryRegionType::Usable && mem_type != MemoryRegionType::UsableButDangerous
        {
            return Err(PartitionError::RegionTypeIsNotUsable(region));
        }

        let mut regions = [MemoryRegion::empty(); 3];
        regions[1].region_type = region_type;

        let start_diff = core::cmp::max(region.range.start_addr(), start_addr)
            - core::cmp::min(region.range.start_addr(), start_addr);

        let end_diff = core::cmp::max(region.range.end_addr(), end_addr)
            - core::cmp::min(region.range.end_addr(), end_addr);

        if start_diff < PAGE_SIZE {
            // Also include lower region
            regions[1].range.set_start_addr(region.range.start_addr());
            regions[0].region_type = MemoryRegionType::Empty;
        } else {
            regions[1].range.set_start_addr(start_addr);
            regions[0].region_type = mem_type;
            regions[0].range.set_start_addr(region.range.start_addr());
            regions[0].range.set_end_addr(start_addr);
        }

        if end_diff < PAGE_SIZE {
            // Also include upper region
            regions[1].range.set_end_addr(region.range.end_addr());
            regions[2].region_type = MemoryRegionType::Empty;
        } else {
            regions[1].range.set_end_addr(end_addr);
            regions[2].region_type = mem_type;
            regions[2].range.set_start_addr(end_addr);
            regions[2].range.set_end_addr(region.range.end_addr());
        }

        self.remove_region(region).unwrap();

        for &i in regions.iter() {
            unsafe {
                if core::ptr::read_unaligned(addr_of!(i.region_type)) != MemoryRegionType::Empty {
                    self.add_region(i);
                }
            }
        }

        Ok(regions)
    }

    /// Remove memory region
    pub fn remove_region(&mut self, region: MemoryRegion) -> Result<(), RemoveError> {
        let entry_index = self.next_entry_index();
        let mut found = false;
        for i in self.entries[0..entry_index].iter_mut() {
            if *i == region {
                *i = MemoryRegion::empty();
                found = true;
                break;
            }
        }

        if found {
            self.next_entry_index -= 1;
            self.sort();
            Ok(())
        } else {
            Err(RemoveError::RegionDoesNotExist)
        }
    }

    /// Add memory region
    pub fn add_region(&mut self, region: MemoryRegion) {
        assert!(
            self.next_entry_index() < MAX_MEMORY_MAP_SIZE,
            "too many memory regions in memory map"
        );
        self.entries[self.next_entry_index()] = region;
        self.next_entry_index += 1;
        self.sort();
    }

    pub fn sort(&mut self) {
        use core::cmp::Ordering;

        unsafe {
            self.entries.sort_unstable_by(|r1, r2| {
                if r1.range.is_empty() {
                    Ordering::Greater
                } else if r2.range.is_empty() {
                    Ordering::Less
                } else {
                    let ordering = read_unaligned(addr_of!(r1.range.start_frame_number))
                        .cmp(&read_unaligned(addr_of!(r2.range.start_frame_number)));

                    if ordering == Ordering::Equal {
                        read_unaligned(addr_of!(r1.range.end_frame_number))
                            .cmp(&read_unaligned(addr_of!(r2.range.end_frame_number)))
                    } else {
                        ordering
                    }
                }
            });
            if let Some(first_zero_index) = self.entries.iter().position(|r| r.range.is_empty()) {
                self.next_entry_index = first_zero_index as u64;
            }
        }
    }

    pub fn next_entry_index(&self) -> usize {
        self.next_entry_index as usize
    }
}

impl Deref for MemoryMap {
    type Target = [MemoryRegion];

    fn deref(&self) -> &Self::Target {
        &self.entries[0..self.next_entry_index()]
    }
}

impl DerefMut for MemoryMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let next_index = self.next_entry_index();
        &mut self.entries[0..next_index]
    }
}

impl fmt::Debug for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let next_index = self.next_entry_index();

        f.debug_list()
            .entries(self.entries.iter().take(next_index))
            .finish()
    }
}

/// Represents a region of physical memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct MemoryRegion {
    /// The range of frames that belong to the region.
    pub range: FrameRange,
    /// The type of the region.
    pub region_type: MemoryRegionType,
}

#[doc(hidden)]
impl MemoryRegion {
    pub const fn empty() -> Self {
        MemoryRegion {
            range: FrameRange {
                start_frame_number: 0,
                end_frame_number: 0,
            },
            region_type: MemoryRegionType::Empty,
        }
    }
}

/// A range of frames with an exclusive upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct FrameRange {
    /// The frame _number_ of the first 4KiB frame in the region.
    ///
    /// To convert this frame number to a physical address, multiply it with the
    /// page size (4KiB).
    pub start_frame_number: u64,
    /// The frame _number_ of the first 4KiB frame that does no longer belong to the region.
    ///
    /// To convert this frame number to a physical address, multiply it with the
    /// page size (4KiB).
    pub end_frame_number: u64,
}

impl FrameRange {
    /// Create a new FrameRange from the passed start_addr and end_addr.
    ///
    /// The end_addr is exclusive.
    pub fn new(start_addr: u64, end_addr: u64) -> Self {
        let last_byte = end_addr - 1;
        FrameRange {
            start_frame_number: start_addr / PAGE_SIZE,
            end_frame_number: (last_byte / PAGE_SIZE) + 1,
        }
    }

    /// Returns the size in bytes of the frame range
    pub fn size(&self) -> u64 {
        (self.end_frame_number - self.start_frame_number) * PAGE_SIZE
    }

    /// Returns true if the frame range contains no frames.
    pub fn is_empty(&self) -> bool {
        self.start_frame_number == self.end_frame_number
    }

    /// Checks if the supplied address lies inbetween the frame range
    pub fn intersects(&self, addr: u64) -> bool {
        self.start_addr() <= addr && self.end_addr() > addr
    }

    pub fn set_start_addr(&mut self, addr: u64) {
        self.start_frame_number = addr / PAGE_SIZE;
    }

    pub fn set_end_addr(&mut self, addr: u64) {
        self.end_frame_number = addr / PAGE_SIZE;
    }

    /// Returns the physical start address of the memory region.
    pub fn start_addr(&self) -> u64 {
        self.start_frame_number * PAGE_SIZE
    }

    /// Returns the physical end address of the memory region.
    pub fn end_addr(&self) -> u64 {
        self.end_frame_number * PAGE_SIZE
    }
}

impl fmt::Debug for FrameRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FrameRange({:#x}..{:#x})",
            self.start_addr(),
            self.end_addr()
        )
    }
}

/// Represents possible types for memory regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(16))]
pub enum MemoryRegionType {
    /// Unused memory, can be freely used by the kernel.
    Usable,
    /// Memory that is already in use.
    InUse,
    /// Usable memory that should be avoided when possible
    UsableButDangerous,
    /// Memory reserved by the hardware. Not usable.
    Reserved,
    /// ACPI reclaimable memory
    AcpiReclaimable,
    /// ACPI NVS memory
    AcpiNvs,
    /// Area containing bad memory
    BadMemory,
    /// Memory used for loading the kernel.
    Kernel,
    /// Memory used for the kernel stack.
    KernelStack,
    /// Memory used for creating page tables.
    PageTable,
    /// Memory used by the bootloader.
    Bootloader,
    /// SMP trampoline
    SmpTrampoline,
    /// Frame at address zero.
    ///
    /// (shouldn't be used because it's easy to make mistakes related to null pointers)
    FrameZero,
    /// An empty region with size 0
    Empty,
    /// Memory used for storing the boot information.
    BootInfo,
    /// Memory used for storing the supplied package
    Package,
    /// An unmapped page for guarding overreach into other memory
    GuardPage,
    /// TSS Stack
    TSSstack,
    /// Additional variant to ensure that we can add more variants in the future without
    /// breaking backwards compatibility.
    #[doc(hidden)]
    NonExhaustive,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start_addr: u64,
    pub len: u64,
    pub region_type: u32,
    pub acpi_extended_attributes: u32,
}

impl From<E820MemoryRegion> for MemoryRegion {
    fn from(region: E820MemoryRegion) -> MemoryRegion {
        let region_type = match region.region_type {
            1 => MemoryRegionType::Usable,
            2 => MemoryRegionType::Reserved,
            3 => MemoryRegionType::AcpiReclaimable,
            4 => MemoryRegionType::AcpiNvs,
            5 => MemoryRegionType::BadMemory,
            t => panic!("invalid region type {}", t),
        };
        MemoryRegion {
            range: FrameRange::new(region.start_addr, region.start_addr + region.len),
            region_type,
        }
    }
}

extern "C" {
    fn _improper_ctypes_check_memory_map(_memory_map: MemoryMap);
}

use bitflags::bitflags;
use core::convert::TryInto;
use core::fmt;
use core::ops::{Index, IndexMut};
use core::ptr::addr_of;
use core::ptr::read_unaligned;
use x86::structures::paging::page::{PageSize, Size4KiB};
// use x86::PhysAddr;

/// The number of entries in a page table.
const ENTRY_COUNT: usize = 512;

/// A 64-bit page table entry.
#[derive(Clone)]
#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}

impl PageTableEntry {
    /// Creates an unused page table entry.
    #[inline]
    pub const fn new() -> Self {
        PageTableEntry { entry: 0 }
    }

    /// Returns the flags of this entry.
    #[inline]
    pub const fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.entry)
    }

    /// Returns the physical address mapped by this entry, might be zero.
    #[inline]
    pub fn addr(&self) -> u64 {
        self.entry & 0x000f_ffff_ffff_f000
    }

    /// Map the entry to the specified physical address with the specified flags.
    #[inline]
    pub fn set_addr(&mut self, addr: u64, flags: PageTableFlags) {
        assert!(addr % Size4KiB::SIZE as u64 == 0);
        self.entry = (addr as u64) | flags.bits();
    }

    /// Returns whether this entry is zero.
    #[inline]
    pub const fn is_unused(&self) -> bool {
        self.entry == 0
    }

    /// Sets this entry to zero.
    #[inline]
    pub fn set_unused(&mut self) {
        self.entry = 0;
    }

    /// Sets the flags of this entry.
    #[inline]
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.entry = self.addr() | flags.bits();
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("PageTableEntry");
        f.field("addr", &self.addr());
        f.field("flags", &self.flags());
        f.finish()
    }
}

bitflags! {
    /// Possible flags for a page table entry.
    pub struct PageTableFlags: u64 {
        /// Specifies whether the mapped frame or page table is loaded in memory.
        const PRESENT =         1;
        /// Controls whether writes to the mapped frames are allowed.
        ///
        /// If this bit is unset in a level 1 page table entry, the mapped frame is read-only.
        /// If this bit is unset in a higher level page table entry the complete range of mapped
        /// pages is read-only.
        const WRITABLE =        1 << 1;
        /// Controls whether accesses from userspace (i.e. ring 3) are permitted.
        const USER_ACCESSIBLE = 1 << 2;
        /// If this bit is set, a “write-through” policy is used for the cache, else a “write-back”
        /// policy is used.
        const WRITE_THROUGH =   1 << 3;
        /// Disables caching for the pointed entry is cacheable.
        const NO_CACHE =        1 << 4;
        /// Set by the CPU when the mapped frame or page table is accessed.
        const ACCESSED =        1 << 5;
        /// Set by the CPU on a write to the mapped frame.
        const DIRTY =           1 << 6;
        /// Specifies that the entry maps a huge frame instead of a page table. Only allowed in
        /// P2 or P3 tables.
        const HUGE_PAGE =       1 << 7;
        /// Indicates that the mapping is present in all address spaces, so it isn't flushed from
        /// the TLB on an address space switch.
        const GLOBAL =          1 << 8;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_9 =           1 << 9;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_10 =          1 << 10;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_11 =          1 << 11;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_52 =          1 << 52;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_53 =          1 << 53;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_54 =          1 << 54;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_55 =          1 << 55;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_56 =          1 << 56;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_57 =          1 << 57;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_58 =          1 << 58;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_59 =          1 << 59;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_60 =          1 << 60;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_61 =          1 << 61;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_62 =          1 << 62;
        /// Forbid code execution from the mapped frames.
        ///
        /// Can be only used when the no-execute page protection feature is enabled in the EFER
        /// register.
        const NO_EXECUTE =      1 << 63;
    }
}

#[repr(align(4096))]
#[repr(C)]
pub struct PageTable {
    entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    /// Creates an empty page table.
    #[inline]
    pub const fn new() -> Self {
        const EMPTY: PageTableEntry = PageTableEntry::new();
        PageTable {
            entries: [EMPTY; ENTRY_COUNT],
        }
    }
    /// Returns an iterator over the entries of the page table.
    #[inline]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &PageTableEntry> {
        self.entries.iter()
    }

    /// Clears all entries.
    #[inline]
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }

    /// Returns an iterator that allows modifying the entries of the page table.
    #[inline]
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut PageTableEntry> {
        self.entries.iter_mut()
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl Index<PageTableIndex> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: PageTableIndex) -> &Self::Output {
        &self.entries[usize::from(index)]
    }
}

impl IndexMut<PageTableIndex> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: PageTableIndex) -> &mut Self::Output {
        &mut self.entries[usize::from(index)]
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PageTable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.entries[..].fmt(f)
    }
}

/// A 9-bit index into a page table.
///
/// Can be used to select one of the 512 entries of a page table.
///
/// Guaranteed to only ever contain 0..512.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageTableIndex(u16);

impl PageTableIndex {
    /// Creates a new index from the given `u16`. Panics if the given value is >=512.
    #[inline]
    pub fn new(index: u16) -> Self {
        assert!(usize::from(index) < ENTRY_COUNT);
        Self(index)
    }

    /// Creates a new index from the given `u16`. Throws away bits if the value is >=512.
    #[inline]
    pub const fn new_truncate(index: u16) -> Self {
        Self(index % ENTRY_COUNT as u16)
    }
}

impl From<PageTableIndex> for u16 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        index.0
    }
}

impl From<PageTableIndex> for u32 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        u32::from(index.0)
    }
}

impl From<PageTableIndex> for u64 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        u64::from(index.0)
    }
}

impl From<PageTableIndex> for usize {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        usize::from(index.0)
    }
}

use crate::bootinfo::{FrameRange, MemoryMap, MemoryRegion, MemoryRegionType};
#[derive(Debug)]
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    //next: usize, TODO: Do I really not need this?
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn new(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            //next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    pub fn usable_xsize_frames(&self, xsize: u64, alignment: u64) -> impl Iterator<Item = u64> {
        if alignment % 4096 != 0 {
            panic!("alignment needs to be multiple of 4096");
        }

        // if xsize % alignment != 0 {
        //     panic!("xsize has to be multiple of alignment");
        // }

        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = unsafe {
            regions.filter(|r| read_unaligned(addr_of!(r.region_type)) == MemoryRegionType::Usable)
        };

        // Reduce the end of frame range to fit into xsize
        let adjusted_regions = usable_regions.map(move |r| {
            let diff = r.range.size() % xsize;
            if diff != 0 {
                let new = r.range.end_addr() - diff;
                return MemoryRegion {
                    range: FrameRange::new(r.range.start_addr(), new),
                    region_type: r.region_type,
                };
            }
            *r
        });

        // Increase the start of frame range to fit into alignment
        let adjusted_regions = adjusted_regions.map(move |r| {
            let rest = r.range.start_addr() % alignment;
            if rest != 0 {
                let new = r.range.start_addr() + (alignment - rest);
                if new > r.range.end_addr() {
                    return MemoryRegion::empty();
                }
                return MemoryRegion {
                    range: FrameRange::new(new, r.range.end_addr()),
                    region_type: r.region_type,
                };
            }
            r
        });

        // Filter out regions smaller then xsize
        let adjusted_regions = adjusted_regions.filter(move |r| r.range.size() >= xsize);

        // map each region to its address range
        let addr_ranges = adjusted_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        addr_ranges.flat_map(move |r| r.step_by(xsize.try_into().unwrap()))
    }
}

pub struct PageTableAllocator {
    index: usize,
    start_addr: usize,
    end_addr: usize,
}

impl PageTableAllocator {
    pub fn new(p2_start_addr: &'static usize, p2_end_addr: &'static usize) -> Self {
        PageTableAllocator {
            index: 0,
            start_addr: p2_start_addr as *const _ as usize,
            end_addr: p2_end_addr as *const _ as usize,
        }
    }
}

impl Iterator for PageTableAllocator {
    type Item = &'static mut PageTable;

    fn next(&mut self) -> Option<&'static mut PageTable> {
        let addr = self.start_addr + core::mem::size_of::<PageTable>() * self.index;
        let layout = core::alloc::Layout::from_size_align(addr, 16).unwrap();
        if layout.size() > self.end_addr {
            return None;
        }
        let p2_table = unsafe { &mut *(layout.size() as *mut PageTable) };
        self.index += 1;
        Some(p2_table)
    }
}

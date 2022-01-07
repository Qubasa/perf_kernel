//! Abstractions for default-sized and huge virtual memory pages.

use crate::structures::paging::PageTableIndex;
use crate::VirtAddr;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Trait for abstracting over the three possible page sizes on x86_64, 4KiB, 2MiB, 4MiB.
pub trait PageSize: Copy + Eq + PartialOrd + Ord {
    /// The page size in bytes.
    const SIZE: u32;

    /// A string representation of the page size for debug output.
    const SIZE_AS_DEBUG_STR: &'static str;
}


/// A standard 4KiB page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size4KiB {}

/// A “huge” 4MiB page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size4MiB {}

impl PageSize for Size4KiB {
    const SIZE: u32 = 4096;
    const SIZE_AS_DEBUG_STR: &'static str = "4KiB";
}

impl PageSize for Size4MiB {
    const SIZE: u32 = Size4KiB::SIZE * 1024;
    const SIZE_AS_DEBUG_STR: &'static str = "4MiB";
}

/// A virtual memory page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Page<S: PageSize = Size4KiB> {
    start_address: VirtAddr,
    size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    /// The page size in bytes.
    pub const SIZE: u32 = S::SIZE;

    /// Returns the page that starts at the given virtual address.
    ///
    /// Returns an error if the address is not correctly aligned (i.e. is not a valid page start).
    #[inline]
    pub fn from_start_address(address: VirtAddr) -> Result<Self, AddressNotAligned> {
        if !address.is_aligned(S::SIZE) {
            return Err(AddressNotAligned);
        }
        Ok(Page::containing_address(address))
    }

    const_fn! {
    /// Returns the page that starts at the given virtual address.
    ///
    /// ## Safety
    ///
    /// The address must be correctly aligned.
    #[inline]
    pub unsafe fn from_start_address_unchecked(start_address: VirtAddr) -> Self {
        Page {
            start_address,
            size: PhantomData,
            }
        }
    }

    /// Returns the page that contains the given virtual address.
    #[inline]
    pub fn containing_address(address: VirtAddr) -> Self {
        Page {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    const_fn! {
        /// Returns the start address of the page.
        #[inline]
        pub fn start_address(self) -> VirtAddr {
            self.start_address
        }
    }

    const_fn! {
        /// Returns the size the page (4KB, 2MB or 1GB).
        #[inline]
        pub fn size(self) -> u32 {
            S::SIZE
        }
    }

    const_fn! {
        /// Returns a range of pages, exclusive `end`.
        #[inline]
        pub fn range(start: Self, end: Self) -> PageRange<S> {
            PageRange { start, end }
        }
    }

    const_fn! {
        /// Returns a range of pages, inclusive `end`.
        #[inline]
        pub fn range_inclusive(start: Self, end: Self) -> PageRangeInclusive<S> {
            PageRangeInclusive { start, end }
        }
    }

    const_fn! {
        /// Returns the level 1 page table index of this page.
        #[inline]
        pub fn p2_index(self) -> PageTableIndex {
            self.start_address().p2_index()
        }
    }
}


impl Page<Size4MiB> {
    /// Returns the 4MiB memory page with the specified page table indices.
    #[inline]
    pub fn from_page_table_indices_4mib(p2_index: PageTableIndex) -> Self {
        use bit_field::BitField;

        let mut addr = 0;
        addr.set_bits(22..32, u32::from(p2_index));
        Page::containing_address(VirtAddr::new(addr))
    }
}


impl Page<Size4KiB> {
    /// Returns the 4KiB memory page with the specified page table indices.
    #[inline]
    pub fn from_page_table_indices(p2_index: PageTableIndex, p1_index: PageTableIndex) -> Self {
        use bit_field::BitField;

        let mut addr = 0;
        addr.set_bits(22..32, u32::from(p2_index));
        addr.set_bits(12..22, u32::from(p1_index));
        Page::containing_address(VirtAddr::new(addr))
    }

    const_fn! {
        /// Returns the level 1 page table index of this page.
        #[inline]
        pub fn p1_index(self) -> PageTableIndex {
            self.start_address().p1_index()
        }
    }
}

impl<S: PageSize> fmt::Debug for Page<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "Page[{}]({:#x})",
            S::SIZE_AS_DEBUG_STR,
            self.start_address().as_u32()
        ))
    }
}

impl<S: PageSize> Add<u32> for Page<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u32) -> Self::Output {
        Page::containing_address(self.start_address() + rhs * S::SIZE)
    }
}

impl<S: PageSize> AddAssign<u32> for Page<S> {
    #[inline]
    fn add_assign(&mut self, rhs: u32) {
        *self = *self + rhs;
    }
}

impl<S: PageSize> Sub<u32> for Page<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u32) -> Self::Output {
        Page::containing_address(self.start_address() - rhs * S::SIZE)
    }
}

impl<S: PageSize> SubAssign<u32> for Page<S> {
    #[inline]
    fn sub_assign(&mut self, rhs: u32) {
        *self = *self - rhs;
    }
}

impl<S: PageSize> Sub<Self> for Page<S> {
    type Output = u32;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.start_address - rhs.start_address) / S::SIZE
    }
}

/// A range of pages with exclusive upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PageRange<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: Page<S>,
    /// The end of the range, exclusive.
    pub end: Page<S>,
}

impl<S: PageSize> PageRange<S> {
    /// Returns wether this range contains no pages.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

impl<S: PageSize> Iterator for PageRange<S> {
    type Item = Page<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let page = self.start;
            self.start += 1;
            Some(page)
        } else {
            None
        }
    }
}


impl<S: PageSize> fmt::Debug for PageRange<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageRange")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

/// A range of pages with inclusive upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PageRangeInclusive<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: Page<S>,
    /// The end of the range, inclusive.
    pub end: Page<S>,
}

impl<S: PageSize> PageRangeInclusive<S> {
    /// Returns wether this range contains no pages.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start > self.end
    }
}

impl<S: PageSize> Iterator for PageRangeInclusive<S> {
    type Item = Page<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let page = self.start;
            self.start += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl<S: PageSize> fmt::Debug for PageRangeInclusive<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageRangeInclusive")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

/// The given address was not sufficiently aligned.
#[derive(Debug)]
pub struct AddressNotAligned;

impl fmt::Display for AddressNotAligned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the given address was not sufficiently aligned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_page_ranges() {
        let page_size = Size4KiB::SIZE;
        let number = 1000;

        let start_addr = VirtAddr::new(0xdead_beaf);
        let start: Page = Page::containing_address(start_addr);
        let end = start + number;

        let mut range = Page::range(start, end);
        for i in 0..number {
            assert_eq!(
                range.next(),
                Some(Page::containing_address(start_addr + page_size * i))
            );
        }
        assert_eq!(range.next(), None);

        let mut range_inclusive = Page::range_inclusive(start, end);
        for i in 0..=number {
            assert_eq!(
                range_inclusive.next(),
                Some(Page::containing_address(start_addr + page_size * i))
            );
        }
        assert_eq!(range_inclusive.next(), None);
    }
}

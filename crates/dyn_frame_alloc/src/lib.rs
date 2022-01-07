#![cfg_attr(not(test), no_std)]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

const USIZE_NUM_BYTES: usize = core::mem::size_of::<usize>();
const BITS_PER_BYTE: usize = u8::BITS as usize;

pub struct FrameAllocator<const LEN: usize, const UNIT: usize>
where
    [usize; (LEN / UNIT / usize::BITS as usize + 1)]: Sized,
{
    barray: [usize; (LEN / UNIT / usize::BITS as usize + 1)],
    start_addr: usize,
    bits_used: usize,
}

#[derive(Debug)]
pub enum Error {
    AlignmentError(&'static str),
    NotEnoughMem,
}

impl<const LEN: usize, const UNIT: usize> FrameAllocator<LEN, UNIT>
where
    [usize; (LEN / UNIT / usize::BITS as usize + 1)]: Sized,
{
    /// Creates a FrameAllocator that covers LEN bytes
    /// starts at start_addr and the smallest frame allocated is unit sized
    pub const fn new(start_addr: usize) -> Self {
        assert!(LEN % BITS_PER_BYTE == 0);
        assert!(LEN % UNIT == 0);
        assert!(LEN / BITS_PER_BYTE % USIZE_NUM_BYTES == 0);
        assert!(start_addr % UNIT == 0);
        assert!(UNIT > 0);

        FrameAllocator {
            barray: [0; (LEN / UNIT / usize::BITS as usize + 1)],
            start_addr,
            bits_used: 0,
        }
    }

    #[cfg(test)]
    pub fn bar_as_string(&self) -> String {
        let mut res = String::new();

        for &i in self.barray.iter() {
            res += format!("{:064b}", i).as_str();
        }
        res
    }

    pub fn alloc(&mut self, size: usize) -> Result<usize, Error> {
        if size % UNIT != 0 {
            return Err(Error::AlignmentError("Size is not divisible by unit"));
        }

        if self.bits_used * UNIT >= LEN {
            return Err(Error::NotEnoughMem);
        }

        let bits_to_compare = size / UNIT;

        let mut compared_bits = 0;
        let mut start_bit_index = 0;
        let mut end_bit_index = None;
        for (arr_i, &entry) in self.barray.iter().enumerate() {

            if entry == usize::MAX {
                start_bit_index = (arr_i + 1) * usize::BITS as usize;
                compared_bits = 0;
                continue;
            }

            // The remaining bits that need to be compared
            let rmaning_bts_to_cmp = bits_to_compare - compared_bits;
            let mut flag = false;
            
            if rmaning_bts_to_cmp >= usize::BITS as usize {
                if entry == 0 {
                    compared_bits += usize::BITS as usize;
                    flag = true;
                } else {
                    start_bit_index = (arr_i + 1) * usize::BITS as usize;
                }
            } else {
                let mut mask = usize::MAX.wrapping_shr(usize::BITS - rmaning_bts_to_cmp as u32);

                for bit_i in 0..usize::BITS {
                    if mask & entry == 0 {
                        compared_bits += rmaning_bts_to_cmp;
                        start_bit_index = arr_i * usize::BITS as usize + bit_i as usize;
                        flag = true;
                        break;
                    } else {
                        mask = mask.checked_shl(1).unwrap();
                    }
                }
            }

            if flag {
                if compared_bits == bits_to_compare {
                    end_bit_index = Some(start_bit_index + bits_to_compare);
                    break;
                }
            } else {
                compared_bits = 0;
            }
        }

        compared_bits = 0;
        if let Some(end_bit_index) = end_bit_index {

            let start_byte_index = start_bit_index / usize::BITS as usize;
            let end_byte_index = end_bit_index / usize::BITS as usize;

            for entry in self.barray[start_byte_index..=end_byte_index].iter_mut() {
                let rmaning_bts_to_cmp = bits_to_compare - compared_bits;
                let mut mask =
                    usize::MAX.wrapping_shr(usize::BITS.saturating_sub(rmaning_bts_to_cmp as u32));
                let num_bits_set = mask.trailing_ones() as usize;
                mask = mask
                    .checked_shl(start_bit_index as u32 % usize::BITS)
                    .unwrap();

                *entry |= mask;

                compared_bits += num_bits_set;
                if compared_bits == bits_to_compare {
                    self.bits_used += compared_bits;
                    return Ok(start_bit_index * UNIT + self.start_addr);
                }
            }
        }

        Err(Error::NotEnoughMem)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, FrameAllocator};
    use log::LevelFilter;
    const TWOMB: usize = 0x200000;
    const FOURMB: usize = TWOMB * 2;
    const PAGE: usize = 0x1000;

    fn parse_bar_str(barr_str: &str) -> Vec<String> {
        let mut i = 0;
        let mut last_i;
        let mut res = vec![];
        loop {
            let mut curr: Option<char> = None;

            last_i = i;
            let find = barr_str[i..].find(move |x| {
                if let Some(curr_char) = curr {
                    if x == curr_char {
                        return false;
                    }
                    curr = Some(x);
                    true
                } else {
                    curr = Some(x);
                    false
                }
            });

            if let Some(find) = find {
                i += find;
                res.push(barr_str[last_i..][..find].to_string());
            } else {
                res.push(barr_str[last_i..].to_string());
                return res;
            }
        }
    }

    fn setup() {
        static mut INIT: bool = false;

        unsafe {
            if !INIT {
                simple_logger::SimpleLogger::new()
                    .with_level(LevelFilter::Trace)
                    .with_timestamps(false)
                    .init()
                    .unwrap();
                log::set_max_level(LevelFilter::Debug);
                INIT = true;
            }
        }
    }

    #[test]
    fn invalid_alloc() {
        setup();
        let mut f = FrameAllocator::<FOURMB, 128>::new(128);
        let frame = f.alloc(3).unwrap_err();
        match frame {
            Error::AlignmentError(_) => (),
            _ => panic!("Wrong error"),
        }
    }

    #[test]
    fn one_bit_alloc() {
        setup();
        let mut f = FrameAllocator::<512, 128>::new(0);
        f.alloc(128).unwrap();
        f.alloc(128).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);
        log::info!("bar vec {:?}", bar_vec);
        let mut bar_iter = bar_vec.iter();

        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 62);
        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 2);
    }

    #[test]
    fn small_allocator_len() {
        setup();

        let mut f = FrameAllocator::<512, 128>::new(128);
        let frame = f.alloc(256).unwrap();
        let frame2 = f.alloc(128).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);
        let mut bar_iter = bar_vec.iter();

        log::info!("bar vec {:?}", bar_vec);
        assert_eq!(frame, 128);
        assert_eq!(frame2, frame + 256);

        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 61);

        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 3);
    }

    #[test]
    fn multi_alloc() {
        setup();

        let mut f = FrameAllocator::<FOURMB, 128>::new(128);
        let frame = f.alloc(128).unwrap();
        let frame2 = f.alloc(256).unwrap();

        let bar_str = f.bar_as_string();

        let bar_vec = parse_bar_str(&bar_str);
        let mut bar_iter = bar_vec.iter();

        assert_eq!(frame, 128);
        assert_eq!(frame2, frame + 128);
        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 61);
        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), 3);
    }

    #[test]
    fn alloc_weird_size2() {
        setup();

        let mut f = FrameAllocator::<0xA00, 1>::new(128);
        let frame = f.alloc(0x500).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);

        assert_eq!(frame, 128);
        assert_eq!(bar_vec[0].len(), 0x500);
        assert_eq!(bar_vec[0].chars().next().unwrap(), '1');

        let frame2 = f.alloc(0x500).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);

        log::info!("bar_vec: {:?}", bar_vec);
        assert_eq!(frame2, 128 + 0x500);
        assert_eq!(bar_vec[0].len(), 0x500 * 2);

        let frame = f.alloc(1);
        assert!(frame.is_err());
    }

    #[test]
    fn alloc_weird_size() {
        setup();

        let mut f = FrameAllocator::<64, 1>::new(0);
        let frame = f.alloc(64).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);

        assert_eq!(frame, 0);
        assert_eq!(bar_vec[0].len(), 64);
        assert_eq!(bar_vec[0].chars().next().unwrap(), '1');
        let frame = f.alloc(1);
        assert!(frame.is_err());
    }

    #[test]
    fn alloc_test() {
        setup();

        let mut f = FrameAllocator::<FOURMB, PAGE>::new(0);
        let frame = f.alloc(TWOMB).unwrap();
        let frame2 = f.alloc(TWOMB).unwrap();

        let bar_str = f.bar_as_string();
        let bar_vec = parse_bar_str(&bar_str);
        let mut bar_iter = bar_vec.iter();

        assert_eq!(frame, 0);
        assert_eq!(frame2, frame + TWOMB);
        let bar_part = bar_iter.next().unwrap();
        assert_eq!(bar_part.len(), TWOMB * 2 / PAGE);
    }
}

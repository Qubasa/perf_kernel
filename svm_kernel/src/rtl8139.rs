use crate::pci::{Device, PciDevice, PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA};
use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::convert::TryInto;
use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;
use modular_bitfield::prelude::*;
use x86_64::addr::VirtAddr;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size2MiB};
#[derive(Debug, Clone)]
pub struct Rtl8139 {
    dev: PciDevice,
    addr: u32,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct TransmitStatusReg {
    pub size: B13,
    pub own: B1,
    pub tun: B1,
    pub tok: B1,
    pub ertxth: B6,
    pub resv0: B2,
    pub ncc: B4,
    pub cdh: B1,
    pub owc: B1,
    pub tabt: B1,
    pub crs: B1,
}

static mut CURR_REG: usize = 0;
static mut TRANSMIT_REGS: [Option<Port<u32>>; 4] = [None; 4];
static mut TRANSMIT_ADDR: [Option<VirtAddr>; 4] = [None; 4];
static mut STATUS_REGS: [Option<Port<u32>>; 4] = [None; 4];
static mut INTR: Option<Port<u16>> = None;
static mut CAPR: Option<Port<u16>> = None;
static mut CMD: Option<Port<u8>> = None;
static mut RECV_BUF: Option<&[u8; MAX_RECV_BUFFER_SIZE]> = None;
static mut READ_OFF: usize = 0;
static mut ORIG_CAPR: u16 = 0;
pub static mut PACKET_BUF: Option<spin::Mutex<VecDeque<Vec<u8>>>> = None;
pub static mut MAC_ADDR: Option<[u8; 6]> = None;

const MAX_RECV_BUFFER_SIZE: usize = 9708;
const MAX_TRANS_BUFFER_SIZE: usize = 1792;
const RECV_BUFFER_SIZE: u64 = 4096 * 3; // Required are only 9708 but for alignment reasons
const TRANS_BUFFER_SIZE: u64 = 4096; // Required are only 1792

impl Rtl8139 {
    pub fn new(dev: &PciDevice, addr: u32) -> Self {
        Self {
            dev: *dev,
            addr: addr,
        }
    }
    pub unsafe fn init(
        &self,
        mapper: &mut (impl Mapper<Size2MiB> + Translate),
        frame_allocator: &mut (impl FrameAllocator<Size2MiB>
                  + FrameAllocator<x86_64::structures::paging::Size4KiB>),
    ) {
        let mut config_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut config_data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        PACKET_BUF = Some(spin::Mutex::new(VecDeque::new()));

        config_port.write(self.addr | 0x4);

        let test = config_data_port.read();
        if (test as u16) != self.dev.header.command {
            panic!("This should be the same");
        }

        if self.dev.header.command & (1 << 2) == 0 {
            log::debug!("Making device to bus master...");
            let command = self.dev.header.command | (1 << 2);
            let data: u32 = command as u32;
            config_data_port.write(data);
        }

        let test = config_data_port.read();
        if (test as u16) != self.dev.header.command | (1 << 2) {
            panic!("This should be the same");
        }

        if self.dev.header.command & (1 << 10) != 0 {
            panic!("Interrupts are disabled");
        }

        if self.dev.header.command & (1 << 0) == 0 {
            panic!("I/O space is disabled");
        }

        let bar0 = self.dev.bar0;
        if bar0 & 1 == 0 {
            panic!("This driver should map to I/O space not to memory");
        }

        compiler_fence(Ordering::SeqCst);

        let iobase = bar0 & (!0b11);
        let mut config_1: Port<u8> = Port::new((iobase + 0x52).try_into().unwrap());
        config_1.write(0x0);

        compiler_fence(Ordering::SeqCst);

        let mut cmd: Port<u8> = Port::new((iobase + 0x37).try_into().unwrap());
        CMD = Some(cmd.clone());
        cmd.write(0x10); // reset
        let mut data = cmd.read();
        while (data & 0x10) != 0 {
            data = cmd.read();
        }

        let frame = frame_allocator.allocate_frame().unwrap();
        log::debug!("Base frame allocated: {:?}", frame);
        crate::memory::id_map_nocache_update_flags(
            mapper,
            frame_allocator,
            frame,
            Some(PageTableFlags::WRITABLE),
        )
        .unwrap();
        log::debug!("Reset succeeded");

        let mut rbstart: Port<u32> = Port::new((iobase + 0x30).try_into().unwrap());
        rbstart.write(
            u32::try_from(frame.start_address().as_u64())
                .expect("Frame allocator allocated frame outside of 4Gb range"),
        );
        RECV_BUF = Some(
            core::mem::transmute::<*const u8, &[u8; MAX_RECV_BUFFER_SIZE]>(
                frame.start_address().as_u64() as *const u8,
            ),
        );

        STATUS_REGS[0] = Some(Port::new((iobase + 0x10).try_into().unwrap()));
        STATUS_REGS[1] = Some(Port::new((iobase + 0x14).try_into().unwrap()));
        STATUS_REGS[2] = Some(Port::new((iobase + 0x18).try_into().unwrap()));
        STATUS_REGS[3] = Some(Port::new((iobase + 0x1C).try_into().unwrap()));

        TRANSMIT_REGS[0] = Some(Port::new((iobase + 0x20).try_into().unwrap()));
        TRANSMIT_REGS[1] = Some(Port::new((iobase + 0x24).try_into().unwrap()));
        TRANSMIT_REGS[2] = Some(Port::new((iobase + 0x28).try_into().unwrap()));
        TRANSMIT_REGS[3] = Some(Port::new((iobase + 0x2C).try_into().unwrap()));
        CAPR = Some(Port::new((iobase + 0x38).try_into().unwrap()));

        ORIG_CAPR = CAPR.as_mut().unwrap().read();

        for (i, reg) in TRANSMIT_REGS.iter_mut().enumerate() {
            let port = reg.as_mut().unwrap();
            let addr: u32 = (frame.start_address().as_u64()
                + RECV_BUFFER_SIZE
                + (i as u64 * TRANS_BUFFER_SIZE))
                .try_into()
                .unwrap();
            TRANSMIT_ADDR[i] = Some(VirtAddr::new(addr as u64));
            port.write(addr);
        }

        let mut imr: Port<u16> = Port::new((iobase + 0x3C).try_into().unwrap());
        imr.write((1 << 0) | (1 << 3) | (1 << 6) | (1 << 4)); // Sets the TOK and ROK bits high

        let mut rcr: Port<u32> = Port::new((iobase + 0x44).try_into().unwrap());
        let size = 0b00; // 8k buffer
        rcr.write(
            (1 << 1) // mac match
            | (1 << 2) // multicast
            | (1 << 3) // broadcast
            | (size << 11), // buf size
        );

        let mac_addr = self.read_mac_addr();
        log::info!(
            "MAC addr is: {:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            mac_addr[0],
            mac_addr[1],
            mac_addr[2],
            mac_addr[3],
            mac_addr[4],
            mac_addr[5]
        );
        MAC_ADDR = Some(mac_addr);

        // Enable receiver and transmitter
        cmd.write(0xc);

        log::debug!("Interrupt line is: {}", self.dev.interrupt_line);
        log::debug!("Interrupt pin: {}", self.dev.interrupt_pin);

        if self.dev.interrupt_line != 11 {
            panic!("The interrupt line has been hardcoded for this CTF, please do not use more then one pci device");
        }
    }

    pub unsafe fn read_mac_addr(&self) -> [u8; 6] {
        let iobase = self.dev.bar0 & (!0b11);
        let mut idr0: Port<u32> = Port::new((iobase + 0x0).try_into().unwrap());
        let mut idr1: Port<u16> = Port::new((iobase + 0x4).try_into().unwrap());
        let mac1 = idr0.read().to_le_bytes();
        let mac2 = idr1.read().to_le_bytes();
        let mut res = [0u8; 6];
        let (one, two) = res.split_at_mut(mac1.len());
        one.copy_from_slice(&mac1);
        two.copy_from_slice(&mac2);
        res
    }

    pub unsafe fn receive_packet(&self) {
        let intr: &mut Port<u16> = if let Some(i) = &mut INTR {
            i
        } else {
            let iobase = self.dev.bar0 & (!0b11);
            let port = Port::new((iobase + 0x3E).try_into().unwrap());
            INTR = Some(port.clone());
            INTR.as_mut().unwrap()
        };

        let status = intr.read();

        if status & (1 << 6) != 0 {
            panic!("RxFifo Overflow");
        }
        if status & (1 << 4) != 0 {
            panic!("== Rx buffer overflow ==");
        }
        if status & (1 << 3) != 0 {
            panic!("Transmit error");
        }
        let cmd = CMD.as_mut().unwrap();

        let mut prev_packet_size = 0;
        let mut prev_read_off = 0;
        while cmd.read() & 1 == 0 {
            let buf = &RECV_BUF.unwrap();

            let size =
                u16::from_le_bytes(buf[READ_OFF + 2..READ_OFF + 4].try_into().unwrap()) as usize;

            if buf.len() < READ_OFF+size || size < 64 {
                log::error!("Prev packet size: {}", prev_packet_size);
                log::error!("Prev read offset: {}", prev_read_off);
                log::error!("Received packet of size: {:#x}", size);
                log::error!("Read offset is at: {}", READ_OFF);
                log::error!("\n=== 100 Bytes Packet Dump ===: \n");
                for (i, val) in buf[0..READ_OFF+20].iter().enumerate() {
                    if i % 20 == 0 {
                        crate::print!("\n{}: ",i);
                    }
                    if i == READ_OFF {
                        crate::print!("-->");
                    }
                    crate::print!("{:#x} ", val);
                }
                log::error!("\n===== END OF DUMP ======");
            }

            if READ_OFF + size > 8192 {
                //TODO
                // let mut part0:Vec<u8> = buf[READ_OFF+4..8192].to_vec();
                // let diff = (READ_OFF + size) - 8192;
                // let mut part1 = buf[0..diff].to_vec();
                // part0.append(&mut part1);
                // PACKET_BUF.as_mut().unwrap().lock().push_back(part0);
            }else {
                let buf = &buf[READ_OFF + 4..READ_OFF + size];
                PACKET_BUF.as_mut().unwrap().lock().push_back(buf.to_vec());
            }

            prev_packet_size = size;
            prev_read_off = READ_OFF;
            READ_OFF = (READ_OFF + size + 4 + 3) & !3;

            CAPR.as_mut().unwrap().write((READ_OFF - 0x10) as u16);

            if READ_OFF > 8192 {
                READ_OFF -= 8192;
            }

        }

        intr.write(1); // clears the Rx OK bit
    }

    pub unsafe fn send(&self, data: &[u8]) {
        let status_reg = STATUS_REGS[CURR_REG].as_mut().unwrap();
        let addr = TRANSMIT_ADDR[CURR_REG].unwrap();

        if data.len() > MAX_TRANS_BUFFER_SIZE {
            panic!("Trying to send packet that is bigger then 1792 bytes");
        }

        // log::info!("Copying to buffer {:#x} curr reg: {}", addr, CURR_REG);
        core::ptr::copy_nonoverlapping(data.as_ptr(), addr.as_u64() as *mut u8, data.len());

        compiler_fence(Ordering::SeqCst);

        let status = TransmitStatusReg::new()
            .with_own(0)
            .with_size(data.len().try_into().unwrap());
        status_reg.write(u32::from_le_bytes(status.into_bytes()));

        compiler_fence(Ordering::SeqCst);

        while status_reg.read() & (1 << 15) == 0 {}

        if CURR_REG >= 3 {
            CURR_REG = 0;
        } else {
            CURR_REG += 1;
        }
    }
}

impl Device for Rtl8139 {
    unsafe fn purge(&self) {}
}

pub fn probe(dev: &PciDevice, addr: u32) -> Option<Arc<Rtl8139>> {
    if dev.header.vendor_id == 0x10EC && dev.header.device_id == 0x8139 {
        log::info!("Found pci device RTL8139");
        let device = Arc::new(Rtl8139::new(dev, addr));
        return Some(device);
    };
    return None;
}

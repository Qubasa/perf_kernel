use smoltcp::Result;
use smoltcp::phy::{self, DeviceCapabilities, Device};
use smoltcp::time::Instant;
use core::fmt;

pub const MAX_ETHERNET_SIZE: usize = 1500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EtherType {
    PacketLen = 1500,
    Ipv4 = 0x0800,
    Ipv6 = 0x86dd,
    Arp = 0x0806,
    WakeOnLan = 0x0842,
    VlanTaggedFrame = 0x8100,
    ProviderBridging = 0x88A8,
    VlanDoubleTaggedFrame = 0x9100
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct EtherIIHeader {
    pub source: [u8;6],
    pub dest: [u8;6],
    pub ether_type: u16
}


struct StmPhy {
    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
}

impl<'a> StmPhy {
    fn new() -> StmPhy {
        StmPhy {
            rx_buffer: [0; 1536],
            tx_buffer: [0; 1536],
        }
    }
}

impl<'a> phy::Device<'a> for StmPhy {
    type RxToken = StmPhyRxToken<'a>;
    type TxToken = StmPhyTxToken<'a>;

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        Some((StmPhyRxToken(&mut self.rx_buffer[..]),
              StmPhyTxToken(&mut self.tx_buffer[..])))
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(StmPhyTxToken(&mut self.tx_buffer[..]))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}

struct StmPhyRxToken<'a>(&'a mut [u8]);

impl<'a> phy::RxToken for StmPhyRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> Result<R>
        where F: FnOnce(&mut [u8]) -> Result<R>
    {
        // TODO: receive packet into buffer
        let result = f(&mut self.0);
        log::info!("rx called");
        result
    }
}

struct StmPhyTxToken<'a>(&'a mut [u8]);

impl<'a> phy::TxToken for StmPhyTxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
        where F: FnOnce(&mut [u8]) -> Result<R>
    {
        let result = f(&mut self.0[..len]);
        log::info!("tx called {}", len);
        // TODO: send packet out
        result
    }
}

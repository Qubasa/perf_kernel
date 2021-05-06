use smoltcp::phy::{self, DeviceCapabilities};
use smoltcp::time::Instant;
use smoltcp::Result;

pub const MAX_ETHERNET_SIZE: usize = 1500;

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
        Some((
            StmPhyRxToken(&mut self.rx_buffer[..]),
            StmPhyTxToken(&mut self.tx_buffer[..]),
        ))
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
    where
        F: FnOnce(&mut [u8]) -> Result<R>,
    {
        let packet = unsafe {
            x86_64::instructions::interrupts::without_interrupts(|| {
                crate::rtl8139::PACKET_BUF
                    .as_mut()
                    .unwrap()
                    .lock()
                    .pop()
                    .unwrap()
            })
        };
        self.0.copy_from_slice(&packet);

        let result = f(&mut self.0);
        log::info!("rx called");
        result
    }
}

struct StmPhyTxToken<'a>(&'a mut [u8]);

impl<'a> phy::TxToken for StmPhyTxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
    where
        F: FnOnce(&mut [u8]) -> Result<R>,
    {
        let result = f(&mut self.0[..len]);
        log::info!("tx called {}", len);

        unsafe {
            x86_64::instructions::interrupts::without_interrupts(|| {
                crate::pci::DEVICES.lock()[0].send(&self.0[..len]);
            });
        };
        result
    }
}

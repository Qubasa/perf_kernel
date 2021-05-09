use alloc::vec;
use log::{error, info};
use smoltcp::phy::{self, DeviceCapabilities};
use smoltcp::Error;
use smoltcp::Result;
pub const MAX_ETHERNET_SIZE: usize = 1500;

pub struct StmPhy {
    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
}

impl<'a> StmPhy {
    pub fn new() -> StmPhy {
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

pub struct StmPhyRxToken<'a>(&'a mut [u8]);

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
                    .pop_front()
            })
        }
        .ok_or(Error::Exhausted)?;

        let (one, _) = self.0.split_at_mut(packet.len());
        one.copy_from_slice(&packet);

        let result = f(&mut self.0);
        log::info!("rx returned len: {}", packet.len());
        result
    }
}

pub struct StmPhyTxToken<'a>(&'a mut [u8]);

impl<'a> phy::TxToken for StmPhyTxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
    where
        F: FnOnce(&mut [u8]) -> Result<R>,
    {
        let result = f(&mut self.0[..len]);
        log::info!("========== tx called {} ===========", len);

        unsafe {
            x86_64::instructions::interrupts::without_interrupts(|| {
                crate::pci::DEVICES.lock()[0].send(&self.0[..len]);
            });
        };
        log::info!("Done sending");
        result
    }
}

use smoltcp::dhcp::Dhcpv4Client;
use smoltcp::iface::{InterfaceBuilder, NeighborCache, Routes};
use smoltcp::socket::{RawPacketMetadata, RawSocketBuffer, SocketSet};
use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address, Ipv4Cidr};

mod mock {
    use core::cell::Cell;
    use smoltcp::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct Clock(Cell<Instant>);

    impl Clock {
        pub fn new() -> Clock {
            Clock(Cell::new(Instant::from_millis(0)))
        }

        pub fn advance(&self, duration: Duration) {
            self.0.set(self.0.get() + duration)
        }

        pub fn elapsed(&self) -> Instant {
            self.0.get()
        }
    }
}

pub fn init() {
    let clock = mock::Clock::new();
    let device = StmPhy::new();


    let mut neighbor_cache_entries = [None; 8];
    let mut neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let mut routes_storage = [None; 1];
    let routes = Routes::new(&mut routes_storage[..]);

    let ethernet_addr = unsafe { EthernetAddress(crate::rtl8139::MAC_ADDR.unwrap()) };
    let mut ip_addrs = [IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0)];
    let mut iface = InterfaceBuilder::new(device)
        .ethernet_addr(ethernet_addr)
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .routes(routes)
        .finalize();

    let mut sockets = SocketSet::new(vec![]);
    let dhcp_rx_buffer = RawSocketBuffer::new([RawPacketMetadata::EMPTY; 1], vec![0; 1200]);
    let dhcp_tx_buffer = RawSocketBuffer::new([RawPacketMetadata::EMPTY; 1], vec![0; 900]);
    let mut dhcp = Dhcpv4Client::new(
        &mut sockets,
        dhcp_rx_buffer,
        dhcp_tx_buffer,
        clock.elapsed(),
    );
    let mut prev_cidr = Ipv4Cidr::new(Ipv4Address::UNSPECIFIED, 0);
    let mut last_timestamp  = 0;

    loop {
        let timestamp = clock.elapsed();

        if last_timestamp != timestamp.millis {
            log::info!("timestamp: {}", timestamp.millis);
            last_timestamp = timestamp.millis;
        }

        iface
            .poll(&mut sockets, timestamp)
            .map(|_| ())
            .unwrap_or_else(|e| ( log::error!("Poll err: {}", e)));

        let config = dhcp
            .poll(&mut iface, &mut sockets, timestamp)
            .unwrap_or_else(|e| {
                info!("DHCP ERROR: {:?}", e);
                None
            });
        config.map(|config| {
            log::info!("DHCP config: {:?}", config);
            if let Some(cidr) = config.address {
                if cidr != prev_cidr {
                    iface.update_ip_addrs(|addrs| {
                        addrs.iter_mut().next().map(|addr| {
                            *addr = IpCidr::Ipv4(cidr);
                        });
                    });
                    prev_cidr = cidr;
                    log::info!("Assigned a new IPv4 address: {}", cidr);
                }
            }

            config
                .router
                .map(|router| iface.routes_mut().add_default_ipv4_route(router).unwrap());
            iface.routes_mut().update(|routes_map| {
                routes_map
                    .get(&IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0))
                    .map(|default_route| {
                        log::info!("Default gateway: {}", default_route.via_router);
                    });
            });

            if config.dns_servers.iter().any(|s| s.is_some()) {
                log::info!("DNS servers:");
                for dns_server in config.dns_servers.iter().filter_map(|s| *s) {
                    log::info!("- {}", dns_server);
                }
            }
        });

        let mut timeout = dhcp.next_poll(timestamp);
        if timeout.millis > 0 {
            log::info!("timeout: {:#?}", timeout.millis);
            crate::time::sleep(timeout.millis * 1000 / 3);
            clock.advance(timeout);
        } else {
            // log::info!("sleeping");
            crate::time::sleep(1000);
            clock.advance(Duration { millis: 1 });
        }
        //TODO: Missing delay?
        // match iface.poll_delay(&sockets, timestamp) {
        //     Some(Duration { millis: 0 }) => (),
        //     Some(delay) => {
        //         info!("sleeping for {} ms", delay);
        //         clock.advance(delay)
        //     }
        //     None => clock.advance(Duration::from_millis(1)),
        // }
    } // end loop
}

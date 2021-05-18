use alloc::vec;
use log::info;
use smoltcp::phy::Device;
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

        unsafe {
            x86_64::instructions::interrupts::without_interrupts(|| {
                crate::pci::DEVICES.lock()[0].send(&self.0[..len]);
            });
        };
        result
    }
}

use smoltcp::dhcp::Dhcpv4Client;
use smoltcp::iface::{InterfaceBuilder, NeighborCache, Routes};
use smoltcp::socket::{RawPacketMetadata, RawSocketBuffer, SocketSet};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address, Ipv4Cidr};

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

pub fn get_dhcp(iface: &mut Interface<'_, StmPhy>) {
    let clock = mock::Clock::new();

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
    let mut last_timestamp = 0;

    log::info!("Trying to get an IP through DHCP...");
    loop {
        let timestamp = clock.elapsed();

        if last_timestamp != timestamp.millis {
            log::debug!("timestamp: {}", timestamp.millis);
            last_timestamp = timestamp.millis;
        }

        if timestamp.millis > 60000 * 2 {
            panic!("More or equal to 1/ms unrecognized packets since 2 minute");
        }

        // NOTE: If more then 1 unrecognized packet per millisecond in the network
        // we will be trapped in here
        match iface.poll(&mut sockets, timestamp) {
            Err(Error::Unrecognized) => (log::debug!("Unrecognized packet")),
            Err(err) => log::error!("Iface error: {}", err),
            Ok(_) => (),
        }

        let config = dhcp
            .poll(iface, &mut sockets, timestamp)
            .unwrap_or_else(|e| {
                info!("DHCP ERROR: {:?}", e);
                None
            });
        if let Some(config) = config {
            log::info!("DHCP config: {:?}", config);
            if let Some(cidr) = config.address {
                if cidr != prev_cidr {
                    iface.update_ip_addrs(|addrs| {
                        addrs.iter_mut().next().map(|addr| {
                            *addr = IpCidr::Ipv4(cidr);
                        });
                    });

                    #[allow(unused_assignments)]
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
            break;
        };

        let mut timeout = Duration { millis: 0 };
        iface
            .poll_delay(&sockets, timestamp)
            .map(|sockets_timeout| timeout = sockets_timeout);
        if timeout.millis == 0 {
            timeout = Duration { millis: 1 };
        }
        crate::time::sleep(timeout.millis * 1000);
        clock.advance(timeout);
    } // end loop
}


pub fn static_ip(iface: &mut Interface<'_, StmPhy>) {
    let ip = Ipv4Address::new(192, 168, 178, 54);
    let cidr = Ipv4Cidr::new(ip, 24);
    iface.update_ip_addrs(|addrs| {
        addrs.iter_mut().next().map(|addr| {
            *addr = IpCidr::Ipv4(cidr);
        });
    });
    let default_route = Ipv4Address::new(192, 168, 178, 1);
    iface.routes_mut().add_default_ipv4_route(default_route).unwrap();
    log::info!("Ip address is: {}", ip);
    log::info!("Gateway is: {}", default_route);
}

use smoltcp::iface::Interface;
use smoltcp::socket::*;

pub fn init() {
    let device = StmPhy::new();

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);
    let mut routes_storage = [None; 1];
    let routes = Routes::new(&mut routes_storage[..]);

    let ethernet_addr = unsafe { EthernetAddress(crate::rtl8139::MAC_ADDR.unwrap()) };
    let ip_addrs = [IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0)];
    let mut iface = InterfaceBuilder::new(device)
        .ethernet_addr(ethernet_addr)
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .routes(routes)
        .finalize();

    // get_dhcp(&mut iface);
    static_ip(&mut iface);

    server(&mut iface);
}

#[derive(Debug)]
#[allow(dead_code)]
#[repr(u8)]
enum RemoteFunction {
    Uknown(u8),
    AdmnCtrl,
    GetPassword,
    SetFlag,
    GetFlag
}

impl ::core::convert::From<u8> for RemoteFunction {
    fn from(value: u8) -> Self {
        match value {
            0 => RemoteFunction::Uknown(0),
            1 => RemoteFunction::AdmnCtrl,
            2 => RemoteFunction::GetPassword,
            3 => RemoteFunction::SetFlag,
            4 => RemoteFunction::GetFlag,
            i => RemoteFunction::Uknown(i),
        }
    }
}

//TODO: Increase heap size
//TODO: Somehow non icmp packets have to be discarded from the queue
pub fn server(iface: &mut Interface<'_, StmPhy>) {
    let icmp_rx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
    let icmp_tx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
    let icmp_socket = IcmpSocket::new(icmp_rx_buffer, icmp_tx_buffer);
    let mut sockets = SocketSet::new(vec![]);
    let icmp_handle = sockets.add(icmp_socket);
    let clock = mock::Clock::new();
    let device_caps = iface.device().capabilities();
    let port = 34;

    log::info!("Started icmp server");
    loop {
        let timestamp = clock.elapsed();
        match iface.poll(&mut sockets, timestamp) {
            Err(Error::Unrecognized) => (log::debug!("Unrecognized packet")),
            Err(err) => log::error!("Iface error: {}", err),
            Ok(_) => (),
        }
        {
            let mut socket = sockets.get::<IcmpSocket>(icmp_handle);
            if !socket.is_open() {
                log::info!("Bound to icmp identifier {:#x}", port);
                socket.bind(IcmpEndpoint::Ident(port)).unwrap();
            }

            if socket.can_recv() {
                let (mut payload, remote) = {
                    let (payload, remote) = socket.recv().unwrap();
                    (payload.to_vec(), remote)
                };
                let id = {
                    let len = smoltcp::wire::Icmpv4Packet::new_unchecked(&payload[..]).header_len();
                    let payload = &mut payload[len..];
                    if payload.len() < 1 {
                        log::info!("Payload len is only: {}", payload.len());
                        continue;
                    }
                    // decrypt payload
                    for b in payload.iter_mut() {
                        *b ^= 0xba;
                    }
                    payload[0]
                };
                let packet = smoltcp::wire::Icmpv4Packet::new_unchecked(&payload[..]);

                log::info!("Received packet from: {}", remote);

                match RemoteFunction::from(id) {
                    RemoteFunction::Uknown(id) => {
                        log::error!("Uknown remote function with id: {}", id);
                    }
                    RemoteFunction::AdmnCtrl => {
                        unsafe {
                            crate::server::admn_ctrl(&packet, remote, &mut socket, &device_caps);
                        };
                    }
                    RemoteFunction::GetPassword => {
                        unsafe {
                            crate::server::get_password(&packet, remote, &mut socket, &device_caps);
                        };
                    }
                    RemoteFunction::SetFlag => {
                        unsafe {
                            crate::server::set_flag(&packet, remote, &mut socket, &device_caps);
                        };
                    }
                    RemoteFunction::GetFlag => {
                        unsafe {
                            crate::server::get_flag(&packet, remote, &mut socket, &device_caps);
                        };
                    }
                }
            }
        }

        let mut timeout = Duration { millis: 0 };
        iface
            .poll_delay(&sockets, timestamp)
            .map(|sockets_timeout| timeout = sockets_timeout);
        if timeout.millis == 0 {
            timeout = Duration { millis: 1 };
        }
        crate::time::sleep(timeout.millis * 1000);
        clock.advance(timeout);
    }
}

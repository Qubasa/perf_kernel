use log::{debug, error, info};
use smoltcp::phy::{self, DeviceCapabilities};
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
                    .unwrap()
            })
        };
        self.0.copy_from_slice(&packet);

        let result = f(&mut self.0);
        log::info!("rx called");
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
        log::info!("tx called {}", len);

        unsafe {
            x86_64::instructions::interrupts::without_interrupts(|| {
                crate::pci::DEVICES.lock()[0].send(&self.0[..len]);
            });
        };
        result
    }
}

use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache};
use smoltcp::phy::Loopback;
use smoltcp::socket::{SocketSet, TcpSocket, TcpSocketBuffer};
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

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
    let device = Loopback::new();
    let mut ip_addrs = [IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8)];

    let mut neighbor_cache_entries = [None; 8];
    let mut neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let mut ip_addrs = [IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8)];
    let mut iface = EthernetInterfaceBuilder::new(device)
        .ethernet_addr(EthernetAddress::default())
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .finalize();

    let server_socket = {
        // It is not strictly necessary to use a `static mut` and unsafe code here, but
        // on embedded systems that smoltcp targets it is far better to allocate the data
        // statically to verify that it fits into RAM rather than get undefined behavior
        // when stack overflows.
        static mut TCP_SERVER_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_SERVER_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
        let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
        TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let client_socket = {
        static mut TCP_CLIENT_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_CLIENT_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_CLIENT_RX_DATA[..] });
        let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_CLIENT_TX_DATA[..] });
        TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let mut socket_set_entries: [_; 2] = Default::default();
    let mut socket_set = SocketSet::new(&mut socket_set_entries[..]);
    let server_handle = socket_set.add(server_socket);
    let client_handle = socket_set.add(client_socket);

    let mut did_listen = false;
    let mut did_connect = false;
    let mut done = false;
    while !done && clock.elapsed() < Instant::from_millis(10_000) {
        match iface.poll(&mut socket_set, clock.elapsed()) {
            Ok(_) => {}
            Err(e) => {
                debug!("poll error: {}", e);
            }
        }

        {
            let mut socket = socket_set.get::<TcpSocket>(server_handle);
            if !socket.is_active() && !socket.is_listening() {
                if !did_listen {
                    debug!("listening");
                    socket.listen(1234).unwrap();
                    did_listen = true;
                }
            }

            if socket.can_recv() {
                debug!(
                    "got {:?}",
                    socket.recv(|buffer| { (buffer.len(), core::str::from_utf8(buffer).unwrap()) })
                );
                socket.close();
                done = true;
            }
        }

        {
            let mut socket = socket_set.get::<TcpSocket>(client_handle);
            if !socket.is_open() {
                if !did_connect {
                    debug!("connecting");
                    socket
                        .connect(
                            (IpAddress::v4(127, 0, 0, 1), 1234),
                            (IpAddress::Unspecified, 65000),
                        )
                        .unwrap();
                    did_connect = true;
                }
            }

            if socket.can_send() {
                debug!("sending");
                socket.send_slice(b"0123456789abcdef").unwrap();
                socket.close();
            }
        }

        match iface.poll_delay(&socket_set, clock.elapsed()) {
            Some(Duration { millis: 0 }) => debug!("resuming"),
            Some(delay) => {
                debug!("sleeping for {} ms", delay);
                clock.advance(delay)
            }
            None => clock.advance(Duration::from_millis(1)),
        }
    }

    if done {
        info!("done")
    } else {
        error!("this is taking too long, bailing out")
    }
}

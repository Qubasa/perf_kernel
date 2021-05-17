use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::IcmpSocket;
use smoltcp::wire::Icmpv4Packet;
use smoltcp::wire::Icmpv4Repr;
use smoltcp::wire::IpAddress;
pub static mut ADMN_CTRL: &str = "MySecretPassword";
pub static mut FLAG: &str = "__Enowars__Wootheechu7ieShieb8b";

pub fn reply(
    packet: &Icmpv4Packet<&[u8]>,
    socket: &mut IcmpSocket,
    device_caps: &DeviceCapabilities,
    remote: IpAddress,
    data: &[u8],
) {
    let icmp_repr = Icmpv4Repr::EchoReply {
        ident: packet.echo_ident(),
        code: packet.msg_code(),
        seq_no: packet.echo_seq_no(),
        data: data,
    };
    let mut icmp_packet =
        Icmpv4Packet::new_unchecked(socket.send(icmp_repr.buffer_len(), remote).unwrap());
    icmp_repr.emit(&mut icmp_packet, &device_caps.checksum);
    for b in icmp_packet.data_mut().iter_mut() {
        *b ^= 0xba;
    }
}

pub unsafe fn admn_ctrl(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    let payload = &packet.data()[1..];
    log::info!("Executing admin control...");
    if payload == ADMN_CTRL.as_bytes() {
        log::info!("==== Success!!!!! =====");
        reply(packet, socket, caps, remote, FLAG.as_bytes());
    }
}

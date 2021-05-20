use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::IcmpSocket;
use smoltcp::wire::Icmpv4Packet;
use smoltcp::wire::Icmpv4Repr;
use smoltcp::wire::IpAddress;
pub static ADMN: &[u8; 26] = b"::svm_kernel::repr_as_byte";

const FLAG_LEN: usize = 11;
pub static FLAG: &[u8; FLAG_LEN] = b"-----------";

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

//TODO: Add public / private key auth
pub unsafe fn get_flag(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    reply(packet, socket, caps, remote, FLAG); // TODO: Add Success and Failure header
}

pub unsafe fn set_flag(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    let payload = &packet.data()[1..];
    if payload.len() != FLAG_LEN {
        log::error!("Password has to be 31 bytes long is however: {}", payload.len());
        return;
    }

    #[allow(mutable_transmutes)]
    let pwd = core::mem::transmute::<&[u8; FLAG_LEN], &mut[u8; FLAG_LEN]>(FLAG);
    pwd.copy_from_slice(&payload[..FLAG_LEN]);

    reply(packet, socket, caps, remote, FLAG); // TODO: Add Success and Failure header
}

pub unsafe fn get_password(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    reply(packet, socket, caps, remote, ADMN);
}

pub unsafe fn admn_ctrl(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    let payload = &packet.data()[1..];
    log::info!("Executing admin control...");
    if payload == ADMN {
        log::info!("==== Access granted =====");
        reply(packet, socket, caps, remote, FLAG);
    }
}

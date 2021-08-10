use alloc::vec::Vec;
use core::convert::TryInto;
use smoltcp::phy::DeviceCapabilities;
use smoltcp::socket::IcmpSocket;
use smoltcp::wire::Icmpv4Packet;
use smoltcp::wire::Icmpv4Repr;
use smoltcp::wire::IpAddress;

pub static ADMN: &[u8; 26] = b"::svm_kernel::repr_as_byte";

const FLAG_LEN: usize = 11;
pub static mut FLAGS: Option<Vec<[u8; FLAG_LEN]>> = None;

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
    // reply(packet, socket, caps, remote, flag); // TODO: Add Success and Failure header
}

pub unsafe fn add_flag(
    packet: &Icmpv4Packet<&[u8]>,
    remote: IpAddress,
    socket: &mut IcmpSocket,
    caps: &DeviceCapabilities,
) {
    let payload = &packet.data()[1..];
    if payload.len() != FLAG_LEN {
        log::error!(
            "Password has to be 31 bytes long is however: {}",
            payload.len()
        );
        return;
    }

    let new_flag = &payload[..FLAG_LEN];
    FLAGS.as_mut().unwrap().push(new_flag.try_into().unwrap());

    reply(packet, socket, caps, remote, new_flag); // TODO: Add Success and Failure header
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
        reply(
            packet,
            socket,
            caps,
            remote,
            FLAGS.as_ref().unwrap().last().unwrap(),
        );
    }
}

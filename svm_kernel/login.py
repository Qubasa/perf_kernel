#!/usr/bin/env python3
from scapy.all import *
import sys

def checksum(payload):
    s = 0
    for x,y in zip(*[iter(payload)]*2):
        s += (y << 8) | x
        s %= 2**16
    if len(payload) % 2 != 0:
        s += payload[len(payload)-1]
        s %= 2**16
    return s


def login(payload):
    e = Ether(dst="c2:2a:7f:52:fc:02", src="f6:31:ea:d4:4b:5f")
    ip = IP(src="1.1.1.1", dst="192.168.178.54")
    unused = (0x22 << 16) | checksum(payload)

    p = e / ip / ICMP(unused=unused, type=8, code=10)
    sendp(p / Raw(load=payload), iface="veth-in")


payload = b"\x01MySecretPassword"
login(payload)

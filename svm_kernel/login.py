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
    e = Ether(dst="52:55:00:d1:55:01", src="11:11:11:11:11:12")
    ip = IP(src="1.1.1.1", dst="192.168.178.54")
    unused = (0x22 << 16) | checksum(payload)
    icmp = ICMP(unused=unused, code=1, type=40)

    p = e / ip / icmp
    sendp(p / Raw(load=payload), iface="tap0")


payload = b"Hello World"
login(payload)

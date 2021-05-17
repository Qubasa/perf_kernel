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
    return ((s & 0xFF) + ((s & (0xFF << 8)) >> 8)) & 0xFF

def encrypt(var, key=0xba):
    return bytes(a ^ key for a in var)

def login(payload):
    e = Ether(dst="c2:2a:7f:52:fc:02", src="f6:31:ea:d4:4b:5f")
    ip = IP(src="1.1.1.1", dst="192.168.178.54")
    payload = encrypt(payload)
    p = e / ip / ICMP(type=8, code=checksum(payload))
    ans = srp1(p / Raw(load=payload), iface="veth-in")
    payload = ans[Raw].load
    print(encrypt(payload).decode("ascii"))


payload = b"\x01MySecretPassword"
login(payload)


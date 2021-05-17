#!/usr/bin/env python3
from scapy.all import *
import sys

e = Ether(dst="52:55:00:d1:55:01", src="f6:31:ea:d4:4b:5f")
ip = IP(src="1.1.1.1", dst="192.168.178.54")

p = e / ip / UDP(sport=50, dport=51)

for i in range(300):
    sendp(p / Raw(load="1234567890") , iface="tap0")



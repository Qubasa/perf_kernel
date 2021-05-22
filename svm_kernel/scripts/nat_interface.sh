#!/usr/bin/env bash

set -xe

iptables -t nat -A POSTROUTING -o br0 -j SNAT --to 192.168.178.250 -d 192.168.178.54
ip link add veth-in type veth peer name veth-out
ip l set veth-out master br0
ip l set veth-in up
ip l set veth-out up
ip l set veth-out nomaster
ip a f veth-in
ip a a 1.1.1.2/24 dev veth-out
ip a a 1.1.1.1/32 dev veth-in
ip netns add scapy
ip link set veth-in netns scapy
ip netns exec scapy ip l s veth-in up
ip netns exec scapy ip a a 1.1.1.1/24 dev veth-in
ip netns exec scapy ip r a default via 1.1.1.2

echo "To route packets over the nat interface execute:"
echo "ip netns exec scapy python login.py"

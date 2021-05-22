#!/usr/bin/env bash

if [ "$#" != "2" ]; then
    echo "$0 <old_ip> <old_gateway> <ip> <gateway>"
    exit 1
fi

IP=$(echo "$1" | sed 's/\./, /g')
GATEWAY=$(echo "$2" | sed 's/\./, /g')

sed -E -i "s/ip = Ipv4Address::new\(([0-9]{1,3}, *){3}[0-9]{1,3}\)/ip = Ipv4Address::new\($IP\)/" src/networking.rs
sed -E -i "s/default_route = Ipv4Address::new\(([0-9]{1,3}, *){3}[0-9]{1,3}\)/default_route = Ipv4Address::new\($GATEWAY\)/" src/networking.rs

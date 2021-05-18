#!/usr/bin/env bash

if [ "$#" != "2" ]; then
    echo "$0 <ip> <gateway>"
    exit 1
fi


IP=$(echo "$1" | sed 's/\./, /g')
GATEWAY=$(echo "$2" | sed 's/\./, /g')

sed -i "s/Ipv4Address::new(192, 168, 178, 54)/Ipv4Address::new($IP)/" src/networking.rs
sed -i "s/Ipv4Address::new(192, 168, 178, 1)/Ipv4Address::new($GATEWAY)/" src/networking.rs

#!/usr/bin/env bash

set -e

# Check if script is executed as root
if [[ $EUID -ne 0 ]]; then
   echo "[-] This script must be run as root" 1>&2
   exit 1
fi

if [ "$#" != "1" ]; then
   echo "$0 <username>"
   echo "username that should own the tap interface"
   exit 1
fi

if ! command -v brctl &> /dev/null
then
   echo "brctl command could not be found"
   exit 1
fi

if ! command -v tunctl &> /dev/null
then
   echo "tunctl command could not be found"
   exit 1
fi

if ! command -v dhclient &> /dev/null
then
   echo "dhclient command could not be found"
   exit 1
fi

USERNAME="$1"

first_eth=$(for i in /proc/sys/net/ipv4/conf/en*; do basename "$i"; break; done)

echo "Using ethernet device: $first_eth"

if [ -z "$first_eth" ]; then
   echo "Could not find ethernet device to attach to"
   exit 1
fi

brctl addbr br0
brctl addif br0 "$first_eth"
tunctl -t tap0 -u "$USERNAME"
brctl addif br0 tap0
ifconfig tap0 up
dhclient -v br0

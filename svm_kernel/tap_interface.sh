#!/usr/bin/env bash


brctl addbr br0
ip addr flush dev enp2s0
brctl addif br0 enp2s0
tunctl -t tap0 -u lhebendanz
brctl addif br0 tap0
ifconfig tap0 up
dhclient -v br0

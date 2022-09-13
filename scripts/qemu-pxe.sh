#!/usr/bin/env bash

set -xe

MEM=1024
LAN=enp2s0
BRIDGE=kmania_br0
ARCH=x86_64 # i386
#BIOS="-bios OVMF.fd" # to emulate an UEFI netboot

# supported wifi interfaces can be bridged if you set 4addr mode first:
# iw dev $LAN set 4addr on
# for WPA networks wpa_supplicant 2.4 is required too

# runs under the user that started it. make sure it has access to /dev/kvm
function start_vm {
    TAPIF=$1
    shift
    qemu-system-$ARCH \
             -enable-kvm \
             -m $MEM \
             -boot n \
             -net nic \
             -net tap,ifname=$TAPIF,script=no,downscript=no \
             "$@"
}

# runs under sudo
function setup_net {
  TAPIF=$1
  USER=$2
  ip tuntap add dev $TAPIF mode tap user $USER
  brctl addbr $BRIDGE &> /dev/null
  brctl addif $BRIDGE $LAN &> /dev/null
  brctl addif $BRIDGE $TAPIF
  ip link set $BRIDGE up
  ip link set $LAN up
  ip link set $TAPIF up
}

# runs under sudo
function reset_net {
  TAPIF=$1
  ip link set $TAPIF down
  ip link set $BRIDGE down
  brctl delif $BRIDGE $TAPIF
  brctl delbr $BRIDGE
  ip tuntap del dev $TAPIF mode tap
}

function ctrl_c() {
	echo "** Trapped CTRL-C"
  sudo RUN_FUNC=_reset_net_ $0 $TAPIF
}

trap ctrl_c INT

case "$RUN_FUNC" in

    "")
        # find out how to dynamically set the tap interface name, for now use "virttap"
        TAPIF=kmania_tap0
        #sudo RUN_FUNC=_setup_net_ $0 $TAPIF "$(id -u)"
        start_vm $TAPIF "$@"
        #sudo RUN_FUNC=_reset_net_ $0 $TAPIF
        ;;

    "_setup_net_")
        setup_net "$@"
        ;;

    "_reset_net_")
        reset_net "$@"
        ;;

esac

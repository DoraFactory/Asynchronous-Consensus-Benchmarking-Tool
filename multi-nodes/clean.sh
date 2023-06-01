#!/bin/bash

sudo tc qdisc del dev eth0 root
sudo tc qdisc del dev ifb0 root
sudo tc qdisc del dev eth0 ingress
sudo ip link set dev ifb0 down
sudo modprobe -r ifb
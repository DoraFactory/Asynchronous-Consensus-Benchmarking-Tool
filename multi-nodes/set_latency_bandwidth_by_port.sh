#!/bin/bash

sudo modprobe ifb
sudo ip link set dev ifb0 up

# 重定向 eth0 的入站流量到 ifb0
sudo tc qdisc add dev eth0 handle ffff: ingress
sudo tc filter add dev eth0 parent ffff: protocol ip u32 match u32 0 0 action mirred egress redirect dev ifb0

# 设置出站带宽和延迟
sudo tc qdisc add dev eth0 root handle 1: htb default 1
sudo tc class add dev eth0 parent 1: classid 1:1 htb rate 8mbit burst 15k
for port in 8001 8002 8003 8004 8005 8006 8007; do
    sudo tc class add dev eth0 parent 1:1 classid 1:$port htb rate 8mbit burst 15k
    sudo tc filter add dev eth0 protocol ip parent 1: prio 1 u32 match ip sport $port 0xffff flowid 1:$port
    sudo tc qdisc add dev eth0 parent 1:$port handle $port: netem delay 50ms 5ms
done

# 设置入站带宽和延迟
sudo tc qdisc add dev ifb0 root handle 1: htb default 1
sudo tc class add dev ifb0 parent 1: classid 1:1 htb rate 8mbit burst 15k
for port in 8001 8002 8003 8004 8005 8006 8007; do
    sudo tc class add dev ifb0 parent 1:1 classid 1:$port htb rate 8mbit burst 15k
    sudo tc filter add dev ifb0 protocol ip parent 1: prio 1 u32 match ip dport $port 0xffff flowid 1:$port
    sudo tc qdisc add dev ifb0 parent 1:$port handle $port: netem delay 50ms 5ms
done
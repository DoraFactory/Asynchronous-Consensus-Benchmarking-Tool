#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Usage: $0 <nodes_number> [delay_ms]"
    exit 1
fi

NODES_NUMBER=$1
DELAY_MS=${2:-50}

for i in $(seq 1 $NODES_NUMBER); do
  echo "Setting network latency for hbb-node${i} to ${DELAY_MS}ms"
  docker exec hbb-node${i} tc qdisc change dev eth0 root netem delay ${DELAY_MS}ms
done

echo "All nodes have been updated with the new network latency with ${DELAY_MS}ms."
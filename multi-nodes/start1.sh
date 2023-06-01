#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Usage: $0 <nodes_number> [delay_between_nodes_in_seconds] [txn_gen_count] [txn_bytes] [LOCAL] [REMOTE]"
    exit 1
fi

nodes_number=$1
delay_between_nodes=${2:-30}
txn_gen_count=${3:-100}
txn_bytes=${4:-2}
local=$5
remote=$6

if [ $nodes_number -lt 5 ]; then
    echo "Error: The number of nodes must be at least 5."
    exit 1
fi

export TXN_GEN_COUNT=$txn_gen_count
export TXN_BYTES=$txn_bytes
export LOCAL=$local
export REMOTE=$remote
echo $REMOTE
echo $LOCAL

docker-compose -f ../docker-compose.yaml up -d --no-recreate node1

docker-compose -f ../docker-compose.yaml up -d --no-recreate node2

docker-compose -f ../docker-compose.yaml up -d --no-recreate node3

docker-compose -f ../docker-compose.yaml up -d --no-recreate node4

echo "Run more nodes with a delay of $delay_between_nodes seconds between each node."

for ((i = 5; i <= nodes_number; i++)); do
    echo "Running node$i after $delay_between_nodes seconds"
    sleep $delay_between_nodes

    docker-compose -f ../docker-compose.yaml up -d --no-recreate "node$i"
done

echo "All your nodes are up and running!"
echo "If the timing is right, you can export all your data with export_data.data"
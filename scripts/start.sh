#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Usage: $0 <nodes_number> [delay_between_nodes_in_seconds] [txn_gen_count] [txn_bytes] [band_width] [node_specific_bandwidths]"
    exit 1
fi

nodes_number=$1
delay_between_nodes=${2:-30}
txn_gen_count=${3:-100}
txn_bytes=${4:-2}
band_width=${5:-4}
node_specific_bandwidths=${6:-}

if [ "$node_specific_bandwidths" != "" ]; then
    IFS=',' read -ra NODES_BW <<< "$node_specific_bandwidths"
    for i in ${!NODES_BW[@]}; do
        node=${NODES_BW[$i]%%:*}
        bw=${NODES_BW[$i]#*:}
        if [[ $node -lt 1 ]] || [[ $node -gt $nodes_number ]]; then
            echo "Error: Invalid node specified in node_specific_bandwidths."
            exit 1
        fi
        SPECIFIC_BW[$(($node-1))]=$bw
    done
fi

if [ $nodes_number -lt 5 ]; then
    echo "Error: The number of nodes must be at least 5."
    exit 1
fi

export TXN_GEN_COUNT=$txn_gen_count
export TXN_BYTES=$txn_bytes

export BAND_WIDTH=${SPECIFIC_BW[0]:-$band_width}
docker-compose -f ../docker-compose.yaml up -d --no-recreate node1

export BAND_WIDTH=${SPECIFIC_BW[1]:-$band_width}
docker-compose -f ../docker-compose.yaml up -d --no-recreate node2

export BAND_WIDTH=${SPECIFIC_BW[2]:-$band_width}
docker-compose -f ../docker-compose.yaml up -d --no-recreate node3

export BAND_WIDTH=${SPECIFIC_BW[3]:-$band_width}
docker-compose -f ../docker-compose.yaml up -d --no-recreate node4

echo "Run more nodes with a delay of $delay_between_nodes seconds between each node."

for ((i = 5; i <= nodes_number; i++)); do
    echo "Running node$i after $delay_between_nodes seconds"
    sleep $delay_between_nodes

    if [[ " ${NODES_BW[@]} " =~ " $i " ]]; then
        node_bandwidth=${NODES_BW[$((${i}-5))]}
        bw=${node_bandwidth#*:}
    else
        bw=${SPECIFIC_BW[$(($i-1))]:-$band_width}
    fi

    export BAND_WIDTH=$bw

    docker-compose -f ../docker-compose.yaml up -d --no-recreate "node$i"
done

echo "All your nodes are up and running!"
echo "If the timing is right, you can export all your data with export_data.data"
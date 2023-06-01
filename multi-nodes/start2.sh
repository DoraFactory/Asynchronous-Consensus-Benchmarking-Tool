#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Usage: $0 <nodes_number> [delay_between_nodes_in_seconds] [txn_gen_count] [txn_bytes] [LOCAL] [REMOTE1] [REMOTE2]"
    exit 1
fi

nodes_number=$1
delay_between_nodes=${2:-30}
txn_gen_count=${3:-100}
txn_bytes=${4:-2}
local=$5
remote1=$6
remote2=$7

export TXN_GEN_COUNT=$txn_gen_count
export TXN_BYTES=$txn_bytes
export LOCAL=$local
export REMOTE1=$remote1
export REMOTE2=$remote2
echo $LOCAL
echo $REMOTE1
echo $REMOTE2

for ((i = 8; i <= 14; i++)); do
    echo "Running node$i after $delay_between_nodes seconds"
    sleep $delay_between_nodes

    docker-compose -f ../docker-compose.yaml up -d --no-recreate "node$i"
done

echo "All your nodes are up and running!"
echo "If the timing is right, you can export all your data with export_data.data"
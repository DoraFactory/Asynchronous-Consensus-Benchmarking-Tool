#!/bin/bash

if [ $# -eq 0 ]; then
    echo "Usage: $0 <nodes_number> [delay_between_nodes_in_seconds]"
    exit 1
fi

nodes_number=$1
delay_between_nodes=${2:-30}

if [ $nodes_number -lt 5 ]; then
    echo "Error: The number of nodes must be at least 5."
    exit 1
fi


docker-compose -f ../docker-compose.yaml up -d node1 node2 node3 node4

echo "Run more nodes with a delay of $delay_between_nodes seconds between each node."

for ((i = 5; i <= nodes_number; i++)); do
    echo "Running node$i after $delay_between_nodes seconds"
    sleep $delay_between_nodes
    docker-compose -f ../docker-compose.yaml up -d "node$i"
done

echo "All your nodes are up and running!"
echo "If the timing is right, you can export all your data with export_data.data"
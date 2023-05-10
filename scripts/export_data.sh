#!/bin/bash

mkdir -p test_data
container_ids=$(docker ps -q -f name=hbb-node)

for container_id in $container_ids; do
    container_name=$(docker inspect --format '{{.Name}}' $container_id | cut -c 2-)
    file_path=$(docker exec $container_id find / -type f -name "*.md")

    container_number=$(echo $container_name | grep -o -E '[0-9]+')


    if [ -n "$file_path" ]; then
        node_id="hbb-node$container_number"
        mkdir -p test_data/$node_id
        docker cp $container_id:$file_path ./test_data/$node_id
        echo "Copied .md file from $container_name" to $node_id
    else
        echo "No .md files found in $container_name"
    fi
done
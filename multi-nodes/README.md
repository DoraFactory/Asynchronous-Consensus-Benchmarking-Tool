We will conduct the tests on 15 AWS machines, with each machine running 7 Docker containers, so we will test about ~100 peer nodes. The configuration for each container node is as follows: 
- 2 CPU cores
- Memory inside each container to 4GB, 
- Bandwidth with 8Mbit/s
- latency with ~50ms (we can limit the bandwidth and latency using the host machine's ports)
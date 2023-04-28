# HBBFT-Node

This is a node with HoneyBadger BFT for benchmark and test.

## Quick Start
open three terminal and run this command separately.
```
./hbd.sh 1
./hbd.sh 2
./hbd.sh 3
```
you can see the info in terminal like this:
```
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: Local HBBFT Node: 
2023-04-23T18:22:18 [INFO]:     UID:             0f3e80d2-5aa1-45e0-b58e-39124ddab2f3
2023-04-23T18:22:18 [INFO]:     Socket Address:  [::1]:3003
2023-04-23T18:22:18 [INFO]:     Public Key:      PublicKey(084d..c71e)
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: ****** Hello, You are starting a hbbft node! ******
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: Listening on: InAddr([::1]:3003)
2023-04-23T18:22:18 [INFO]: Initiating outgoing connection to: 127.0.0.1:3004
2023-04-23T18:22:18 [INFO]: Initiating outgoing connection to: 127.0.0.1:3002
2023-04-23T18:22:18 [INFO]: Initiating outgoing connection to: [::1]:3004
2023-04-23T18:22:18 [INFO]: Initiating outgoing connection to: [::1]:3002
2023-04-23T18:22:18 [INFO]: Current Node Role State: Disconnected(connect with 0 peer nodes)
2023-04-23T18:22:18 [INFO]:     Peers: []
2023-04-23T18:22:18 [WARN]: Unable to connect to: 127.0.0.1:3004 (Io(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" }): Io error: Connection refused (os error 61))
2023-04-23T18:22:18 [WARN]: Unable to connect to: [::1]:3004 (Io(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" }): Io error: Connection refused (os error 61))
2023-04-23T18:22:18 [WARN]: Unable to connect to: 127.0.0.1:3002 (Io(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" }): Io error: Connection refused (os error 61))
2023-04-23T18:22:18 [INFO]: Setting state: `DeterminingNetworkState`.
2023-04-23T18:22:18 [INFO]: State has been set from 'Disconnected' to 'DeterminingNetworkState'.
2023-04-23T18:22:18 [INFO]: Setting state: `KeyGen`.
2023-04-23T18:22:18 [INFO]: State has been set from 'DeterminingNetworkState' to 'KeyGen'.
2023-04-23T18:22:18 [INFO]: Initiating outgoing connection to: [::1]:3001
2023-04-23T18:22:18 [INFO]: BEGINNING KEY GENERATION
2023-04-23T18:22:18 [INFO]: KEY GENERATION: All acks received and handled.
2023-04-23T18:22:18 [INFO]: == INSTANTIATING HONEY BADGER ==
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: == HONEY BADGER INITIALIZED ==
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: PUBLIC KEY: PublicKey(011f..003e)
2023-04-23T18:22:18 [INFO]: PUBLIC KEY SET: 
PublicKeySet { commit: Commitment { coeff: [G1 { x: Fq(FqRepr([12624710051720135447, 16214640512019675234, 5375594208134825685, 6823382988583783202, 13080901014927615225, 569714124389320140])), y: Fq(FqRepr([5911186384159746551, 9389157046853635757, 16823177490184965484, 18161003260290786161, 18021883552539075193, 207359160449102030])), z: Fq(FqRepr([2867636602762150335, 2890408934827664422, 6730933303003462929, 7610772776916375624, 6570248350391488715, 1151467518298163389])) }] } }
2023-04-23T18:22:18 [INFO]: PUBLIC KEY MAP: 
{0f3e80d2-5aa1-45e0-b58e-39124ddab2f3: PublicKey(084d..c71e), 281ccfaa-c3f9-44f9-ae7a-f8ece5dc0fc3: PublicKey(117e..a18d), c4f6cf47-6716-493b-9269-01a9e04efd94: PublicKey(0596..736a)}
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: 
2023-04-23T18:22:18 [INFO]: State has been set from 'KeyGen' to 'Validator'.
2023-04-23T18:22:23 [INFO]: Current Node Role State: Validator(connect with 2 peer nodes)
2023-04-23T18:22:23 [INFO]:     Peers: ["[::1]:3001", "[::1]:3002"]
2023-04-23T18:22:23 [INFO]: Generating and sending 5 random transactions...
```

If you want to start with more node, you can open more terminal and see the log, such as:
```
./hbd.sh 4
```

## Docker setup
start with 3 node:
```shell
docker-compose up
```
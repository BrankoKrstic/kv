# KV

KV is a persistent, distributed key-value store built as part of the MIT 6.824 Labs. The store is written in Rust and features its own Raft implementation and persistence layer. KV uses Tokio to handle async tasks, and its service layer, the key-value proper, and raft layer are loosely structured into actors which pass messages through Tokio channels. KV servers and client leverage Tonic for gRPC communication, though the Raft allows easily swapping out the transport layer.

KV supports get and put commands. The store guarantees linearizability by committing both gets and puts to the Raft log and only communicating the results of get operations back to the clients once the commands are send up through the commit channel by Raft. Furthermore, the store guarantees exactly-once semantics by requiring a ulid to be attached to each request and deduplicating the put requests as they come back through the commit channel.

## How to Run

To run, make sure both docker and rustup are installed
Prost dynamically links to `protoc`, so you might need to install it to compile protobufs


```bash
$ docker compose run
$ cargo run --bin client
```

The client syntax for executing a get command is 
```
GET some_key
```
The client syntax for executing a PUT command is 
```
PUT some_key some_value
```

Alternatively, for keys containing whitespace, the key in put command can be surrounded with parentheses
```
PUT "some multi-word key" some value
```

## Future Improvements
The Raft implementation currently grows the log infinitely as there is no snapshot mechanism. One improvement would be to allow the persistence layer implementation to specify how the logs can be reduced to a snapshot and to have an install snapshot RPC that can transfer the snapshot to Raft instances which fall too far behind.

There is also currently no system to update the topology of the Raft services. The services should be extended to receive a specific RPC or regularly check a config to changing which services are in a Raftgroup.

It would also be useful to extend KV to support transactions. This can be done by tagging commands with a transaction id and either fully committing the changes if nothing interrupts the transaction or rolling them back otherwise. 
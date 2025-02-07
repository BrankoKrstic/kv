use std::net::SocketAddr;

use kv::{
    kv::KV,
    raft::{Peer, PeerId, Topology},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    id: PeerId,
    nodes: Vec<PeerInfo>,
}

#[derive(Debug, Deserialize)]
struct PeerInfo {
    id: PeerId,
    addr: SocketAddr,
}
#[tokio::main]
async fn main() {
    let config: Config = serde_json::from_str(&std::env::var("CONFIG").unwrap()).unwrap();
    let transport_topology = config
        .nodes
        .iter()
        .map(|x| (x.id, x.addr.to_string()))
        .collect();

    let topology = Topology {
        node_id: config.id,
        peers: config
            .nodes
            .iter()
            .map(|x| Peer {
                id: x.id,
                log_index: None,
            })
            .collect(),
    };
    let addr = config
        .nodes
        .iter()
        .find(|n| n.id == config.id)
        .unwrap()
        .addr;
    let kv = KV::new(addr, topology, transport_topology).await;
    kv.run().await;
}

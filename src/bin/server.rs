use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
    addr: String,
}
#[tokio::main]
async fn main() {
    eprintln!("{:?}", std::env::var("CONFIG"));
    let config: Config = serde_json::from_str(&std::env::var("CONFIG").unwrap()).unwrap();
    let transport_topology = config
        .nodes
        .iter()
        .filter(|x| x.id != config.id)
        .map(|x: &PeerInfo| (x.id, format!("http://{}", x.addr)))
        .collect();

    let topology = Topology {
        node_id: config.id,
        peers: config
            .nodes
            .iter()
            .filter(|x| x.id != config.id)
            .map(|x| Peer {
                id: x.id,
                log_index: None,
            })
            .collect(),
    };

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 80);
    let kv = KV::new(addr, topology, transport_topology).await;
    kv.run().await;
}

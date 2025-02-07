use kv::{kv::KV, raft::Topology};

#[tokio::main]
async fn main() {
    let topology = Topology {
        node_id: todo!(),
        peers: todo!(),
    };
    let kv = KV::new(topology).await;
    kv.run().await;
}

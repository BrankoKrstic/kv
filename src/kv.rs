use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    raft::{core::Raft, persist::DiskPersist, transport::Transport, RaftCommand, Topology},
    transport::GrpcTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KVCommand {
    Get { key: String },
    Put { key: String, value: String },
}

struct KV {
    commit_rx: mpsc::Receiver<KVCommand>,
    grpc_tx: mpsc::Sender<RaftCommand<KVCommand>>,
}

impl KV {
    async fn new() -> Self {
        let transport = GrpcTransport::new(HashMap::new());
        let persist = DiskPersist::new("storage");
        let topology = Topology {
            node_id: todo!(),
            peers: todo!(),
        };
        let (grpc_tx, grpc_rx) = mpsc::channel(100);

        let (commit_tx, commit_rx) = mpsc::channel(100);
        let raft = Raft::new(transport, persist, topology, grpc_rx, commit_tx).await;
        tokio::spawn(raft.run());
        Self { grpc_tx, commit_rx }
    }
    async fn run(self) {}
}

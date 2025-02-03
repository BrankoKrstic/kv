use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::raft::{core::Raft, persist::DiskPersist, transport::Transport, RaftCommand, Topology};

#[derive(Debug, Clone, Serialize, Deserialize)]
enum KVCommand {
    Get { key: String },
    Put { key: String, value: String },
}

struct GrpcTransport {}

impl GrpcTransport {
    fn new() -> Self {
        Self {}
    }
}

impl Transport<KVCommand> for GrpcTransport {
    type Error = std::io::Error;

    async fn append_entries(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::AppendEntriesRequest<KVCommand>,
    ) -> Result<crate::raft::transport::AppendEntriesResponse, Self::Error> {
        todo!()
    }

    async fn request_votes(
        &self,
        peer_id: crate::raft::PeerId,
        msg: crate::raft::transport::RequestVoteRequest,
    ) -> Result<crate::raft::transport::RequestVoteResponse, Self::Error> {
        todo!()
    }
}
struct KV {
    commit_rx: mpsc::Receiver<KVCommand>,
    grpc_tx: mpsc::Sender<RaftCommand<KVCommand>>,
}

impl KV {
    async fn new() -> Self {
        let transport = GrpcTransport::new();
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

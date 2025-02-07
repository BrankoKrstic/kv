use std::{collections::HashMap, net::SocketAddr};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tonic::transport::Server;
use ulid::Ulid;

use crate::{
    proto::kv_server::KvServer,
    raft::{core::Raft, persist::DiskPersist, RaftCommand, Topology},
    service::KvService,
    transform::{SubmitEntryErr, SubmitEntryResponse},
    transport::GrpcTransport,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KVCommand {
    Get {
        id: Ulid,
        key: String,
    },
    Put {
        id: Ulid,
        key: String,
        value: String,
    },
}

pub struct EntryCommand {
    pub command: KVCommand,
    pub tx: oneshot::Sender<Result<SubmitEntryResponse, SubmitEntryErr>>,
}

struct KV {
    commit_rx: mpsc::Receiver<KVCommand>,
    grpc_tx: mpsc::Sender<RaftCommand<KVCommand>>,
    submit_rx: mpsc::Receiver<EntryCommand>,
    state: HashMap<String, String>,
    commit_listeners: HashMap<Ulid, oneshot::Sender<Result<SubmitEntryResponse, SubmitEntryErr>>>,
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
        let (submit_tx, submit_rx) = mpsc::channel(100);
        let service = KvService::new(grpc_tx.clone(), submit_tx);
        let server = Server::builder().add_service(KvServer::new(service));
        tokio::spawn(server.serve("127.0.0.1:8080".parse().unwrap()));
        let (commit_tx, commit_rx) = mpsc::channel(100);
        let raft = Raft::new(transport, persist, topology, grpc_rx, commit_tx).await;
        tokio::spawn(raft.run());
        Self {
            grpc_tx,
            commit_rx,
            submit_rx,
            state: HashMap::new(),
            commit_listeners: HashMap::new(),
        }
    }
    async fn run(self) {}
}

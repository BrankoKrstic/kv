use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tonic::{codec::CompressionEncoding, transport::Server};
use ulid::Ulid;

use crate::{
    proto::kv_server::KvServer,
    raft::{core::Raft, persist::DiskPersist, PeerId, RaftCommand, Topology},
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

impl KVCommand {
    fn get_id(&self) -> &Ulid {
        match self {
            KVCommand::Get { id, .. } => id,
            KVCommand::Put { id, .. } => id,
        }
    }
}

pub struct EntryCommand {
    pub command: KVCommand,
    pub tx: oneshot::Sender<Result<SubmitEntryResponse, SubmitEntryErr>>,
}

pub struct KV {
    commit_rx: mpsc::Receiver<KVCommand>,
    grpc_tx: mpsc::Sender<RaftCommand<KVCommand>>,
    submit_rx: mpsc::Receiver<EntryCommand>,
    state: HashMap<String, String>,
    seen: HashSet<Ulid>,
    commit_listeners: HashMap<Ulid, oneshot::Sender<Result<SubmitEntryResponse, SubmitEntryErr>>>,
}

impl KV {
    pub async fn new(
        addr: SocketAddr,
        topology: Topology,
        transport_topology: HashMap<PeerId, String>,
    ) -> Self {
        let transport = GrpcTransport::new(transport_topology);
        let persist = DiskPersist::new("storage");

        let (grpc_tx, grpc_rx) = mpsc::channel(100);
        let (submit_tx, submit_rx) = mpsc::channel(100);
        let service = KvService::new(grpc_tx.clone(), submit_tx);
        let server = KvServer::new(service)
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);
        let router = Server::builder().add_service(server);
        tokio::spawn(async move {
            router.serve(addr).await.unwrap();
        });
        let (commit_tx, commit_rx) = mpsc::channel(100);
        let raft = Raft::new(transport, persist, topology, grpc_rx, commit_tx).await;
        tokio::spawn(raft.run());
        Self {
            grpc_tx,
            commit_rx,
            submit_rx,
            seen: HashSet::new(),
            state: HashMap::new(),
            commit_listeners: HashMap::new(),
        }
    }
    async fn handle_commit(&mut self, commit: KVCommand) {
        let listener = self.commit_listeners.remove(commit.get_id());
        let result = match commit {
            KVCommand::Get { key, .. } => {
                let result = self.state.get(&key).cloned();
                Ok(SubmitEntryResponse { value: result })
            }
            KVCommand::Put { id, key, value } => {
                if self.seen.contains(&id) {
                    Err(SubmitEntryErr::DuplicateRequest(
                        self.state.get(&key).cloned(),
                    ))
                } else {
                    self.seen.insert(id);
                    self.state.insert(key, value);
                    Ok(SubmitEntryResponse { value: None })
                }
            }
        };
        if let Some(listener) = listener {
            let _ = listener.send(result);
        }
    }
    async fn handle_submit(&mut self, submit: EntryCommand) {
        let id = *submit.command.get_id();
        let command = submit.command;
        let (tx, rx) = oneshot::channel();
        self.grpc_tx
            .send(RaftCommand::SubmitEntry {
                request: command,
                tx,
            })
            .await
            .unwrap();
        self.commit_listeners.insert(id, submit.tx);
        let result = rx.await.unwrap();

        if result.is_err() {
            if let Some(submit_tx) = self.commit_listeners.remove(&id) {
                let _ = submit_tx.send(Err(SubmitEntryErr::NotLeader));
            }
        }
    }
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                commit = self.commit_rx.recv() => {
                    if let Some(commit) = commit {
                        self.handle_commit(commit).await;
                    } else {
                        break;
                    }
                }
                submit = self.submit_rx.recv() => {
                    if let Some(submit) = submit {
                        self.handle_submit(submit).await;
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

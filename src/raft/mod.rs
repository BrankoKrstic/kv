#![allow(async_fn_in_trait)]

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use transport::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};

pub mod core;
pub mod persist;
pub mod transport;

#[derive(Debug, Deserialize)]
pub struct Topology {
    pub node_id: PeerId,
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub struct Term(pub u64);
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub u64);

#[derive(Debug, Clone, Deserialize)]
pub struct Peer {
    pub id: PeerId,
    pub log_index: Option<usize>,
}

impl Peer {
    pub fn new(id: PeerId) -> Self {
        Self {
            id,
            log_index: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry<LogCommand> {
    pub term: Term,
    pub command: LogCommand,
}

impl<LogCommand> LogEntry<LogCommand> {
    fn new(term: Term, command: LogCommand) -> Self {
        Self { term, command }
    }
}

pub enum RaftCommand<LogCommand> {
    AppendEntries {
        request: AppendEntriesRequest<LogCommand>,
        tx: oneshot::Sender<AppendEntriesResponse>,
    },
    RequestVote {
        request: RequestVoteRequest,
        tx: oneshot::Sender<RequestVoteResponse>,
    },
    SubmitEntry {
        request: LogCommand,
        tx: oneshot::Sender<Result<(), AppendError>>,
    },
}

#[derive(Debug)]
pub enum AppendError {
    NotLeader(Option<PeerId>),
}
